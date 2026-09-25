//! What each run of a lens is handed: one block and the blocks around it,
//! or the whole of what the lens looks at — as JSON for a command, or as a
//! message for an agent.

use arto_config::{Lens, LensDisplay};
use arto_markdown::{BlockKind, SourcePosition, SourceRange};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Version of the JSON a command reads, for a command to check against.
const INPUT_VERSION: u32 = 1;

/// A block the page offered, in document order.
///
/// The page sends every block a lens could look at, not only the ones it is
/// asked to, because the neighbours of a target are the context the command
/// is given.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Block {
    /// The id the page tagged the element with, to put the answer back.
    pub id: String,
    pub kind: BlockKind,
    pub range: SourceRange,
    /// Where the block's own content ends early (see
    /// [`arto_markdown::block_content`]).
    #[serde(default)]
    pub until: Option<SourcePosition>,
    /// Whether the lens looks at this block, or it is only context.
    #[serde(default)]
    pub target: bool,
}

/// What one run looks at.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum Request {
    /// One block, with its neighbours as context.
    Block(BlockRequest),
    /// The whole document, or the text of one block.
    Whole(WholeRequest),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockRequest {
    version: u32,
    lens: String,
    kind: BlockKind,
    markdown: String,
    range: SourceRange,
    before: Vec<String>,
    after: Vec<String>,
    document: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WholeRequest {
    version: u32,
    lens: String,
    /// Where `markdown` is in the file; `None` for the whole document.
    range: Option<SourceRange>,
    markdown: String,
    document: PathBuf,
}

impl Request {
    /// The JSON a command reads on stdin.
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(self).expect("a request serializes")
    }

    /// What an agent is asked: the reader's `prompt`, then the text.
    ///
    /// The whole of a document is handed over bare, after a blank line,
    /// because that is the one shape every model reads as "the text": a
    /// model trained for one task — a translation model — translates any
    /// framing around the text along with it and hands back the tags. Only a
    /// block with neighbours is framed, since its context has to be told
    /// apart from it. Without a prompt there is nothing to tell apart, and
    /// the text is all there is: such a model reads an instruction as more
    /// text too.
    pub(crate) fn to_message(&self, prompt: Option<&str>) -> String {
        let Some(prompt) = prompt.map(str::trim).filter(|prompt| !prompt.is_empty()) else {
            let text = match self {
                Request::Whole(whole) => &whole.markdown,
                Request::Block(block) => &block.markdown,
            };
            return format!("{}\n", text.trim_end());
        };
        let mut message = format!("{prompt}\n\n");
        match self {
            Request::Whole(whole) => {
                message.push_str(whole.markdown.trim_end());
                message.push('\n');
            }
            Request::Block(block) => {
                let kind = serde_json::to_string(&block.kind).expect("a kind serializes");
                // The file's name alone: where it lives on this machine —
                // the reader's name, a client's, a project's — is nothing
                // the agent needs, and an agent may be a service elsewhere.
                let name = block
                    .document
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let _ = writeln!(
                    message,
                    "The text is one {} of the Markdown document {name}. The blocks around it are given for context only.",
                    kind.trim_matches('"').replace('-', " "),
                );
                if !block.before.is_empty() {
                    section(&mut message, "before", &block.before.join("\n\n"));
                }
                section(&mut message, "text", &block.markdown);
                if !block.after.is_empty() {
                    section(&mut message, "after", &block.after.join("\n\n"));
                }
            }
        }
        message
    }
}

fn section(message: &mut String, name: &str, body: &str) {
    let _ = write!(message, "\n<{name}>\n{}\n</{name}>\n", body.trim_end());
}

/// What an agent of a lens shown as `display` is sent for `prompt`, with
/// placeholders for the text: built by the same code that builds the real
/// message, so that what the preferences show a reader cannot drift from
/// what the agent gets.
pub(crate) fn message_shape(display: LensDisplay, prompt: Option<&str>) -> String {
    let placeholder = |what: &str| format!("…{what}…");
    let range = SourceRange {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 1 },
    };
    let document = PathBuf::from("document.md");
    let request = if display.is_per_block() {
        Request::Block(BlockRequest {
            version: INPUT_VERSION,
            lens: String::new(),
            kind: BlockKind::Paragraph,
            markdown: placeholder("the block"),
            range,
            before: vec![placeholder("the blocks before it")],
            after: vec![placeholder("the blocks after it")],
            document,
        })
    } else {
        Request::Whole(WholeRequest {
            version: INPUT_VERSION,
            lens: String::new(),
            range: None,
            markdown: placeholder("the document, or the block asked about"),
            document,
        })
    };
    request.to_message(prompt)
}

/// One run of the command.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Job {
    /// The block the answer belongs to; empty for a run over the document.
    pub id: String,
    pub request: Request,
    /// Identifies the answer: the same lens on the same text of the same
    /// document gives the same answer, wherever in it the text has moved.
    pub cache_key: [u8; 32],
}

/// The per-block runs `lens` needs for the target `blocks`.
///
/// `content` reads a block's content out of the source; a block that has
/// none (a range the file no longer has) is neither run nor used as
/// context.
pub(crate) fn per_block(
    lens: &Lens,
    document: &Path,
    blocks: &[Block],
    content: impl Fn(&Block) -> Option<String>,
) -> Vec<Job> {
    let readable: Vec<(&Block, String)> = blocks
        .iter()
        .filter_map(|block| content(block).map(|text| (block, text)))
        .collect();
    let texts = |range: std::ops::Range<usize>| -> Vec<String> {
        readable[range]
            .iter()
            .map(|(_, text)| text.clone())
            .collect()
    };

    readable
        .iter()
        .enumerate()
        .filter(|(_, (block, _))| block.target)
        .map(|(index, (block, markdown))| {
            let after_end = index
                .saturating_add(1)
                .saturating_add(lens.context)
                .min(readable.len());
            let request = BlockRequest {
                version: INPUT_VERSION,
                lens: lens.id.clone(),
                kind: block.kind,
                markdown: markdown.clone(),
                range: block.range,
                before: texts(index.saturating_sub(lens.context)..index),
                after: texts(index + 1..after_end),
                document: document.to_path_buf(),
            };
            let mut key = KeyBuilder::new(lens, document);
            key.field(&serde_json::to_string(&request.kind).expect("a kind serializes"));
            key.range(lens, Some(request.range));
            key.field(&request.markdown);
            key.fields(&request.before);
            key.fields(&request.after);
            Job {
                id: block.id.clone(),
                request: Request::Block(request),
                cache_key: key.finish(),
            }
        })
        .collect()
}

/// The one run of `lens` over `markdown`: the whole document, or the text
/// of the block at `range`, whose answer belongs to the block `id`.
pub(crate) fn whole(
    lens: &Lens,
    document: &Path,
    markdown: &str,
    range: Option<SourceRange>,
    id: &str,
) -> Job {
    let mut key = KeyBuilder::new(lens, document);
    key.field(markdown);
    key.range(lens, range);
    Job {
        id: id.to_string(),
        request: Request::Whole(WholeRequest {
            version: INPUT_VERSION,
            lens: lens.id.clone(),
            range,
            markdown: markdown.to_string(),
            document: document.to_path_buf(),
        }),
        cache_key: key.finish(),
    }
}

/// The key an answer is cached under: everything that decides how the
/// command is run, the document, and the text. The range goes in only for a
/// command, which is handed it: an agent is never told where the text is, so
/// for it an edit above a block moves the block without changing its answer.
struct KeyBuilder(Sha256);

impl KeyBuilder {
    fn new(lens: &Lens, document: &Path) -> Self {
        let mut key = Self(Sha256::new());
        key.field(&lens.id);
        key.field(&serde_json::to_string(&lens.agent).expect("an agent serializes"));
        key.field(lens.model.as_deref().unwrap_or_default());
        key.field(lens.prompt.as_deref().unwrap_or_default());
        key.field(
            &lens
                .program
                .as_deref()
                .unwrap_or(Path::new(""))
                .to_string_lossy(),
        );
        key.fields(&lens.command);
        key.field(lens.system.as_deref().unwrap_or_default());
        key.field(lens.endpoint.as_deref().unwrap_or_default());
        key.field(
            &lens
                .context_length
                .map(|length| length.to_string())
                .unwrap_or_default(),
        );
        key.field(&document.to_string_lossy());
        key
    }

    fn range(&mut self, lens: &Lens, range: Option<SourceRange>) {
        if lens.agent.is_none() {
            self.field(&range.map(|range| range.to_string()).unwrap_or_default());
        }
    }

    /// Length-prefixed, so that no two sequences of fields collide.
    fn field(&mut self, value: &str) {
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value.as_bytes());
    }

    fn fields(&mut self, values: &[impl AsRef<str>]) {
        self.field(&values.len().to_string());
        for value in values {
            self.field(value.as_ref());
        }
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lens(context: usize) -> Lens {
        Lens {
            id: "explain".to_string(),
            label: "Explain".to_string(),
            display: LensDisplay::Annotate,
            agent: None,
            model: None,
            prompt: None,
            program: None,
            system: None,
            endpoint: None,
            api_key_command: Vec::new(),
            allow: Vec::new(),
            context_length: None,
            command: vec!["explain".to_string()],
            context,
            concurrency: 1,
            timeout_seconds: 1,
            unit: Default::default(),
            shortcut: None,
        }
    }

    fn block(id: &str, line: usize, target: bool) -> Block {
        Block {
            id: id.to_string(),
            kind: BlockKind::Paragraph,
            range: format!("{line}:1-{line}:3").parse().unwrap(),
            until: None,
            target,
        }
    }

    /// Content is the block id in upper case; `gone` has none.
    fn content(block: &Block) -> Option<String> {
        (block.id != "gone").then(|| block.id.to_uppercase())
    }

    fn json(job: &Job) -> serde_json::Value {
        serde_json::from_str(&job.request.to_json()).unwrap()
    }

    #[test]
    fn a_target_gets_its_neighbours_as_context() {
        let blocks = [
            block("a", 1, false),
            block("b", 3, false),
            block("c", 5, true),
            block("d", 7, false),
        ];

        let jobs = per_block(&lens(2), Path::new("/doc.md"), &blocks, content);

        assert_eq!(jobs.len(), 1);
        let input = json(&jobs[0]);
        assert_eq!(jobs[0].id, "c");
        assert_eq!(input["version"], 1);
        assert_eq!(input["lens"], "explain");
        assert_eq!(input["kind"], "paragraph");
        assert_eq!(input["markdown"], "C");
        assert_eq!(input["range"], "5:1-5:3");
        assert_eq!(input["before"], serde_json::json!(["A", "B"]));
        assert_eq!(input["after"], serde_json::json!(["D"]));
        assert_eq!(input["document"], "/doc.md");
    }

    #[test]
    fn a_block_without_content_is_neither_run_nor_context() {
        let blocks = [
            block("a", 1, false),
            block("gone", 3, true),
            block("c", 5, true),
        ];

        let jobs = per_block(&lens(1), Path::new("/doc.md"), &blocks, content);

        assert_eq!(
            jobs.iter().map(|job| job.id.as_str()).collect::<Vec<_>>(),
            ["c"]
        );
        assert_eq!(json(&jobs[0])["before"], serde_json::json!(["A"]));
    }

    #[test]
    fn a_huge_context_is_all_there_is() {
        let blocks = [block("a", 1, false), block("b", 3, true)];

        let jobs = per_block(&lens(usize::MAX), Path::new("/doc.md"), &blocks, content);

        assert_eq!(json(&jobs[0])["before"], serde_json::json!(["A"]));
    }

    #[test]
    fn a_key_follows_what_runs_the_text_the_document_and_what_the_runner_is_told() {
        let here = [block("a", 1, false), block("b", 3, true)];
        let moved = [block("a", 11, false), block("b", 13, true)];
        let key = |blocks: &[Block], lens: &Lens, document: &str| {
            per_block(lens, Path::new(document), blocks, content)[0].cache_key
        };
        let base = key(&here, &lens(1), "/doc.md");

        // An agent is never told where the text is, so a block moved by an
        // edit above it keeps its answer; a command is, and may use it.
        let mut agent = lens(1);
        agent.set_agent(Some(arto_config::LensAgent::Claude));
        assert_eq!(
            key(&here, &agent, "/doc.md"),
            key(&moved, &agent, "/doc.md")
        );
        assert_ne!(base, key(&moved, &lens(1), "/doc.md"));
        assert_ne!(base, key(&here, &lens(0), "/doc.md"));
        assert_ne!(base, key(&here, &lens(1), "/other.md"));
        let mut other_command = lens(1);
        other_command.command.push("--fast".to_string());
        assert_ne!(base, key(&here, &other_command, "/doc.md"));
        let mut other_prompt = lens(1);
        other_prompt.prompt = Some("Be brief.".to_string());
        assert_ne!(base, key(&here, &other_prompt, "/doc.md"));
        for change in [
            (|lens: &mut Lens| lens.system = Some("Answer in Japanese.".to_string()))
                as fn(&mut Lens),
            |lens| lens.endpoint = Some("http://localhost:1234/v1".to_string()),
            |lens| lens.context_length = Some(8192),
        ] {
            let mut server = lens(1);
            change(&mut server);
            assert_ne!(base, key(&here, &server, "/doc.md"), "{server:?}");
        }
    }

    #[test]
    fn a_whole_run_hands_over_the_text_and_where_it_is() {
        let document = whole(&lens(0), Path::new("/doc.md"), "# Doc\n", None, "");
        let block = whole(
            &lens(0),
            Path::new("/doc.md"),
            "| a |",
            Some("3:1-5:5".parse().unwrap()),
            "b1",
        );

        assert_eq!(json(&document)["markdown"], "# Doc\n");
        assert_eq!(json(&document)["range"], serde_json::Value::Null);
        assert_eq!(json(&block)["range"], "3:1-5:5");
        assert_eq!(block.id, "b1");
        assert_ne!(document.cache_key, block.cache_key);
    }

    #[test]
    fn without_a_prompt_the_text_is_all_there_is() {
        let document = whole(
            &lens(0),
            Path::new("/doc.md"),
            "# Doc\n\nText.\n\n",
            None,
            "",
        );
        assert_eq!(document.request.to_message(None), "# Doc\n\nText.\n");
        assert_eq!(document.request.to_message(Some("  ")), "# Doc\n\nText.\n");

        let blocks = [block("a", 1, false), block("b", 3, true)];
        let jobs = per_block(&lens(1), Path::new("/doc.md"), &blocks, content);
        assert_eq!(jobs[0].request.to_message(None), "B\n");
    }

    #[test]
    fn an_agent_is_handed_a_document_bare_and_a_block_with_its_context_set_apart() {
        let document = whole(&lens(0), Path::new("/doc.md"), "# Doc\n", None, "");
        assert_eq!(
            document.request.to_message(Some("  Translate it.\n")),
            "Translate it.\n\n# Doc\n"
        );

        let blocks = [block("a", 1, false), block("b", 3, true)];
        let jobs = per_block(
            &lens(1),
            Path::new("/Users/someone/Private/doc.md"),
            &blocks,
            content,
        );
        assert_eq!(
            jobs[0].request.to_message(Some("Explain it.")),
            concat!(
                "Explain it.\n\n",
                "The text is one paragraph of the Markdown document doc.md. ",
                "The blocks around it are given for context only.\n",
                "\n<before>\nA\n</before>\n",
                "\n<text>\nB\n</text>\n",
            )
        );
    }

    #[test]
    fn the_page_offers_blocks_as_json() {
        let block: Block = serde_json::from_str(
            r#"{"id": "b3", "kind": "list-item", "range": "3:1-4:9", "until": "4:3", "target": true}"#,
        )
        .unwrap();

        assert_eq!(block.kind, BlockKind::ListItem);
        assert_eq!(block.until, Some(SourcePosition { line: 4, column: 3 }));
        assert!(block.target);
    }

    #[test]
    fn a_block_lens_shows_its_prompt_above_the_block_and_its_neighbours() {
        let shape = message_shape(LensDisplay::Annotate, Some("Explain <text>."));

        assert!(shape.starts_with("Explain <text>.\n\n"), "{shape}");
        let before = shape.find("<before>").unwrap();
        let text = shape.find("<text>\n…the block…").unwrap();
        let after = shape.find("<after>").unwrap();
        assert!(before < text && text < after, "{shape}");
    }

    #[test]
    fn a_lens_over_a_whole_hands_the_text_over_bare_after_its_prompt() {
        for display in [LensDisplay::Page, LensDisplay::Popover] {
            let shape = message_shape(display, Some("Summarize."));

            assert_eq!(
                shape,
                "Summarize.\n\n…the document, or the block asked about…\n"
            );
        }
    }

    #[test]
    fn without_a_prompt_the_text_is_all_that_is_sent() {
        assert_eq!(message_shape(LensDisplay::Annotate, None), "…the block…\n");
    }
}
