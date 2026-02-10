use std::path::{Path, PathBuf};
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo, Utf32String};
use nucleo_matcher::{Matcher, Utf32Str};

use super::directory_search::{
    collect_candidates, collect_file_candidates, enrich_results_with_context, DirectorySearchMatch,
    LineCandidate,
};
use crate::markdown::source_maps;

/// Maximum number of results to extract per Nucleo instance
const MAX_RESULTS: usize = 100;

/// Command sent from Dioxus UI to the worker thread
pub(crate) enum NucleoCommand {
    /// Set the current file for File mode search (re-injects file_nucleo)
    SetFile { file: PathBuf },
    /// Set the root directory for Directory mode search (re-injects dir_nucleo)
    SetDirectory { dir: PathBuf },
    /// Update the search pattern (reparses both Nucleo instances)
    Reparse { query: String },
    /// Shut down the worker thread (sent on window close)
    Stop,
}

/// Results sent from the worker thread back to Dioxus UI
#[derive(Default)]
pub(crate) struct NucleoResults {
    /// Results from file_nucleo (current file lines)
    pub file_results: Vec<DirectorySearchMatch>,
    /// Results from dir_nucleo (all files in directory)
    pub dir_results: Vec<DirectorySearchMatch>,
}

/// Main worker loop that owns two Nucleo instances and processes commands.
///
/// Runs on a dedicated std::thread because Nucleo needs exclusive &mut self
/// access for tick() and pattern.reparse(). The two instances allow File mode
/// searches to complete in <1ms regardless of directory size.
pub(crate) fn nucleo_worker(
    cmd_rx: std::sync::mpsc::Receiver<NucleoCommand>,
    result_tx: tokio::sync::mpsc::UnboundedSender<NucleoResults>,
) {
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let mut file_nucleo: Nucleo<LineCandidate> =
        Nucleo::new(Config::DEFAULT, notify.clone(), None, 1);
    let mut dir_nucleo: Nucleo<LineCandidate> = Nucleo::new(Config::DEFAULT, notify, None, 1);

    // Track last injected paths to skip redundant re-injection
    let mut last_file: Option<PathBuf> = None;
    let mut last_dir: Option<PathBuf> = None;
    let mut current_query = String::new();

    // Cache last extracted results to avoid recomputing when only one instance changed
    let mut cached_file_results: Vec<DirectorySearchMatch> = Vec::new();
    let mut cached_dir_results: Vec<DirectorySearchMatch> = Vec::new();

    loop {
        // 1. Process all pending commands (non-blocking drain)
        let mut had_commands = false;
        while let Ok(cmd) = cmd_rx.try_recv() {
            had_commands = true;
            match cmd {
                NucleoCommand::SetFile { file } => {
                    if Some(&file) == last_file.as_ref() {
                        continue;
                    }
                    last_file = Some(file.clone());
                    file_nucleo.restart(true);
                    inject_file_candidates(&file_nucleo, &file);
                    // Re-apply current pattern after re-injection
                    if !current_query.is_empty() {
                        reparse_pattern(&mut file_nucleo, &current_query);
                    }
                }
                NucleoCommand::SetDirectory { dir } => {
                    if Some(&dir) == last_dir.as_ref() {
                        continue;
                    }
                    last_dir = Some(dir.clone());
                    dir_nucleo.restart(true);
                    inject_directory_candidates(&dir_nucleo, &dir);
                    // Re-apply current pattern after re-injection
                    if !current_query.is_empty() {
                        reparse_pattern(&mut dir_nucleo, &current_query);
                    }
                }
                NucleoCommand::Reparse { query } => {
                    current_query = query;
                    reparse_pattern(&mut file_nucleo, &current_query);
                    reparse_pattern(&mut dir_nucleo, &current_query);
                }
                NucleoCommand::Stop => return,
            }
        }

        // 2. Tick both Nucleo instances (10ms timeout each)
        let file_status = file_nucleo.tick(10);
        let dir_status = dir_nucleo.tick(10);

        // 3. If either produced new results, re-extract only the changed one and send both
        if file_status.changed || dir_status.changed {
            if file_status.changed {
                cached_file_results =
                    extract_top_results(file_nucleo.snapshot(), &current_query, MAX_RESULTS);
                enrich_results_with_context(&mut cached_file_results);
            }
            if dir_status.changed {
                cached_dir_results =
                    extract_top_results(dir_nucleo.snapshot(), &current_query, MAX_RESULTS);
                enrich_results_with_context(&mut cached_dir_results);
            }
            let _ = result_tx.send(NucleoResults {
                file_results: cached_file_results.clone(),
                dir_results: cached_dir_results.clone(),
            });
        }

        // 4. Sleep briefly when idle to avoid busy-spinning
        if !had_commands && !file_status.running && !dir_status.running {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
}

/// Inject all lines from a single file into a Nucleo instance.
fn inject_file_candidates(nucleo: &Nucleo<LineCandidate>, file: &Path) {
    let injector = nucleo.injector();
    for candidate in collect_file_candidates(file) {
        let text: Utf32String = candidate.line_text.as_str().into();
        injector.push(candidate, |_, cols| {
            cols[0] = text;
        });
    }
}

/// Inject all lines from a directory tree into a Nucleo instance.
fn inject_directory_candidates(nucleo: &Nucleo<LineCandidate>, dir: &Path) {
    let injector = nucleo.injector();
    for candidate in collect_candidates(dir) {
        let text: Utf32String = candidate.line_text.as_str().into();
        injector.push(candidate, |_, cols| {
            cols[0] = text;
        });
    }
}

/// Reparse the pattern on a Nucleo instance with smart case detection.
fn reparse_pattern(nucleo: &mut Nucleo<LineCandidate>, query: &str) {
    let case = if query.chars().any(|c| c.is_uppercase()) {
        CaseMatching::Respect
    } else {
        CaseMatching::Ignore
    };
    nucleo
        .pattern
        .reparse(0, query, case, Normalization::Smart, false);
}

/// Extract top results from a Nucleo snapshot, computing match indices for highlighting.
///
/// Nucleo's high-level `matched_items()` provides pre-sorted items but without
/// character-level match positions or individual scores. We use
/// `nucleo_matcher::Matcher::fuzzy_indices()` to compute both the score and
/// exact highlight positions needed by the UI.
fn extract_top_results(
    snapshot: &nucleo::Snapshot<LineCandidate>,
    query: &str,
    max_results: usize,
) -> Vec<DirectorySearchMatch> {
    let matched_count = snapshot.matched_item_count() as usize;
    let count = matched_count.min(max_results);
    if count == 0 || query.is_empty() {
        return Vec::new();
    }

    // Set up nucleo-matcher for computing score + match indices
    let ignore_case = !query.chars().any(|c| c.is_uppercase());
    let mut matcher_config = nucleo_matcher::Config::DEFAULT;
    matcher_config.ignore_case = ignore_case;
    let mut matcher = Matcher::new(matcher_config);

    let mut needle_buf = Vec::new();
    let needle = Utf32Str::new(query, &mut needle_buf);
    let mut haystack_buf = Vec::new();
    let mut indices_buf = Vec::new();

    let mut results = Vec::with_capacity(count);

    // matched_items() returns Item { data, matcher_columns } pre-sorted by score
    for item in snapshot.matched_items(0..count as u32) {
        let candidate = item.data;

        // Compute score + match indices using nucleo-matcher
        haystack_buf.clear();
        indices_buf.clear();
        let haystack = Utf32Str::new(&candidate.line_text, &mut haystack_buf);
        let score = matcher
            .fuzzy_indices(haystack, needle, &mut indices_buf)
            .unwrap_or(0);

        // Convert source character indices to content character indices
        let segments = source_maps::build_line_source_map(&candidate.line_text);
        let content_match_indices =
            source_maps::convert_source_to_content_indices(&segments, &indices_buf);

        results.push(DirectorySearchMatch {
            file_path: candidate.file_path.clone(),
            line_number: candidate.line_number,
            line_text: candidate.line_text.clone(),
            score: score as u32,
            match_indices: indices_buf.clone(),
            content_match_indices,
            context_before: Vec::new(),
            context_after: Vec::new(),
        });
    }

    results
}
