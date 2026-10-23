mod copy_as;
mod copy_code_as;
mod copy_path_as;
mod copy_table_as;
mod data;
mod highlight_ops;
mod image_ops;
mod lens_ops;
mod source_ops;

pub use data::*;

use crate::components::popover::Popover;
use dioxus::document;
use dioxus::prelude::*;
use rust_i18n::t;
use std::path::Path;

use super::context_menu_state::ContentContextMenuState;
use crate::components::context_menu::{ContextMenuItem, ContextMenuSeparator};
use crate::components::icon::IconName;
use crate::keybindings::dispatcher::dispatch_action;
use crate::keybindings::{shortcut_hint_for_context_action, Action, KeyContext};
use crate::utils::source_lines::format_location;
use crate::utils::task::spawn_detached;
use copy_as::CopyAsSubmenu;
use copy_code_as::CopyCodeAsSubmenu;
use copy_path_as::CopyPathAsSubmenu;
use copy_table_as::CopyTableAsSubmenu;
use highlight_ops::HighlightItems;
use image_ops::{CopyImageAsSubmenu, CopySpecialBlockAsSubmenu};
use lens_ops::LensItems;
use source_ops::LinkContextItems;

#[component]
pub fn ContentContextMenu(menu: ContentContextMenuState, on_close: EventHandler<()>) -> Element {
    let ContentContextMenuState {
        data:
            ContextMenuData {
                context,
                x,
                y,
                has_selection,
                selected_text,
                source_line,
                source_line_end,
                table_csv,
                table_tsv,
                table_markdown,
                table_source_line,
                table_source_line_end,
                highlight_ids,
                highlight_under_pointer,
            },
        current_file,
        base_dir: _,
    } = menu;
    let shortcut = |action| shortcut_hint_for_context_action(KeyContext::Content, action);
    let state = use_context::<crate::state::AppState>();

    // Extract copyable source from context (code blocks, mermaid, math)
    let (copy_code_source, code_block_line, code_block_line_end) = match &context {
        ContentContext::CodeBlock {
            content,
            source_line,
            source_line_end,
            ..
        } => (Some(content.clone()), *source_line, *source_line_end),
        ContentContext::Mermaid { source } | ContentContext::MathBlock { source } => {
            // The renderer sets source_line/source_line_end to the block's
            // line range for mermaid/math (via detectContext's block-level override)
            (Some(source.clone()), source_line, source_line_end)
        }
        _ => (None, None, None),
    };

    // Extract image info for smart default and submenu
    let image_info = match &context {
        ContentContext::Image { src, alt } => Some((src.clone(), alt.clone())),
        _ => None,
    };
    let is_image = image_info.is_some();

    // Detect special blocks (Mermaid/Math) for image operations
    let (is_mermaid, is_math) = match &context {
        ContentContext::Mermaid { .. } => (true, false),
        ContentContext::MathBlock { .. } => (false, true),
        _ => (false, false),
    };
    let is_special_block = is_mermaid || is_math;

    let has_context_specific = matches!(
        context,
        ContentContext::Link { .. }
            | ContentContext::Image { .. }
            | ContentContext::Mermaid { .. }
            | ContentContext::MathBlock { .. }
    );

    let has_table = table_csv.is_some();
    let has_file = current_file.is_some();
    let has_lenses = state.rendered_source.read().is_some()
        && (!crate::lenses::offered_lenses().is_empty()
            || !crate::lenses::open_runs(&state).is_empty());
    let has_any_submenu = has_selection
        || has_file
        || copy_code_source.is_some()
        || has_table
        || is_image
        || is_special_block;

    let copy_path = copy_path_row(current_file.as_deref(), source_line, source_line_end);

    rsx! {
        Popover {
            backdrop_class: "context-menu-backdrop",
            class: "context-menu content-context-menu",
            style: format!("left: {x}px; top: {y}px;"),
            keep_selection: true,
            contain_clicks: true,
            on_close,

            // === Section 1: Smart default copy operations ===
            if has_selection {
                ContextMenuItem {
                    label: t!("context_menu.copy").to_string(),
                    icon: Some(IconName::Copy),
                    on_click: {
                        let on_close = on_close;
                        let text = selected_text.clone();
                        move |_| {
                            crate::utils::clipboard::copy_text(&text);
                            on_close.call(());
                        }
                    },
                }
            }

            if let Some(code_content) = copy_code_source.clone() {
                CopyItem {
                    label: t!("context_menu.copy_code").to_string(),
                    shortcut: shortcut("clipboard.copy_code"),
                    text: code_content,
                    on_close,
                }
            }

            // Copy Table (smart default: TSV)
            if let Some(tsv) = table_tsv.clone() {
                CopyItem {
                    label: t!("context_menu.copy_table").to_string(),
                    shortcut: shortcut("clipboard.copy_table_as_tsv"),
                    text: tsv,
                    on_close,
                }
            }

            // Copy Image (smart default: transparent background)
            if let Some((ref src, _)) = image_info {
                ContextMenuItem {
                    label: t!("context_menu.copy_image").to_string(),
                    shortcut: shortcut("clipboard.copy_image"),
                    icon: Some(IconName::Photo),
                    on_click: {
                        let on_close = on_close;
                        let src = src.clone();
                        move |_| {
                            let src = src.clone();
                            spawn_detached(async move {
                                crate::keybindings::dispatcher::copy_image_from_src(src, false).await;
                            });
                            on_close.call(());
                        }
                    },
                }
            }

            // Copy Image for Mermaid/Math blocks (default: transparent)
            if is_special_block {
                ContextMenuItem {
                    label: t!("context_menu.copy_image").to_string(),
                    shortcut: shortcut("clipboard.copy_image"),
                    icon: Some(IconName::Photo),
                    on_click: {
                        let on_close = on_close;
                        move |_| {
                            copy_special_block_image(is_mermaid, false);
                            on_close.call(());
                        }
                    },
                }
            }

            // Copy Path (smart default: path / path:line / path:start-end)
            if let Some(row) = copy_path {
                CopyItem {
                    label: row.label,
                    shortcut: shortcut(row.action),
                    text: row.value,
                    on_close,
                }
            }

            // === Section 2: Selection and search ===
            ContextMenuSeparator {}

            ContextMenuItem {
                label: t!("context_menu.select_all").to_string(),
                icon: Some(IconName::SelectAll),
                on_click: {
                    let on_close = on_close;
                    move |_| {
                        exec_edit_command("selectAll");
                        on_close.call(());
                    }
                },
            }

            ActionItem {
                label: t!("context_menu.find_in_page").to_string(),
                icon: IconName::Search,
                action: Action::SearchOpen,
                on_close,
            }

            // === The reader's highlights ===
            if has_selection || !highlight_ids.is_empty() || highlight_under_pointer.is_some() {
                ContextMenuSeparator {}
                HighlightItems {
                    has_selection: has_selection && state.rendered_source.read().is_some(),
                    highlight_ids: highlight_ids.clone(),
                    highlight_under_pointer: highlight_under_pointer.clone(),
                    on_close: on_close,
                }
            }

            // === The document itself ===
            //
            // A right-click that lands in the margin, or on the band above the
            // page, is about the document rather than about anything in it —
            // and so is one that lands on a paragraph, once the paragraph's own
            // offers have been made. These are the same things the panel offers
            // on the row that opened it.
            if state.current_file().is_some() {
                ContextMenuSeparator {}

                ActionItem {
                    label: t!("context_menu.copy_file_path").to_string(),
                    icon: IconName::Copy,
                    action: Action::CopyFilePath,
                    on_close,
                }

                ActionItem {
                    label: t!("context_menu.reveal_in_finder").to_string(),
                    icon: IconName::Folder,
                    action: Action::FileRevealInFinder,
                    on_close,
                }

                ActionItem {
                    label: t!("context_menu.reload").to_string(),
                    icon: IconName::Refresh,
                    action: Action::WindowReload,
                    on_close,
                }
            }

            // === Section 3: Copy As... submenus ===
            if has_any_submenu {
                ContextMenuSeparator {}
            }

            // Copy As... (Text / Markdown)
            if has_selection {
                CopyAsSubmenu {
                    selected_text: selected_text.clone(),
                    current_file: current_file.clone(),
                    source_line: source_line,
                    source_line_end: source_line_end,
                    on_close: on_close,
                }
            }

            // Copy Path As... (Path / Path with Line / Path with Range)
            if has_file {
                CopyPathAsSubmenu {
                    current_file: current_file.clone().unwrap(),
                    source_line: source_line,
                    source_line_end: source_line_end,
                    on_close: on_close,
                }
            }

            // Copy Code As... (Code / Markdown / Path with Range)
            if let Some(code_content) = copy_code_source.clone() {
                CopyCodeAsSubmenu {
                    code_content,
                    current_file: current_file.clone(),
                    block_source_line: code_block_line,
                    block_source_line_end: code_block_line_end,
                    on_close: on_close,
                }
            }

            // Copy Table As... (TSV / CSV / Markdown)
            if has_table {
                CopyTableAsSubmenu {
                    table_tsv: table_tsv.clone(),
                    table_csv: table_csv.clone(),
                    table_markdown: table_markdown.clone(),
                    current_file: current_file.clone(),
                    table_source_line: table_source_line,
                    table_source_line_end: table_source_line_end,
                    on_close: on_close,
                }
            }

            // Copy Image As... (Image / Markdown / Path)
            if let Some((ref src, ref alt)) = image_info {
                CopyImageAsSubmenu {
                    src: src.clone(),
                    alt: alt.clone(),
                    on_close: on_close,
                }
            }

            // Copy Image As... (Image / Image with Background) for special blocks
            if is_special_block {
                CopySpecialBlockAsSubmenu {
                    is_mermaid,
                    on_close: on_close,
                }
            }

            // === Section 4: Context-specific items (link, image) ===
            if has_context_specific {
                ContextMenuSeparator {}
            }

            match &context {
                ContentContext::Link { href } => rsx! {
                    LinkContextItems {
                        href: href.clone(),
                        on_close: on_close,
                    }
                },
                ContentContext::Image { .. }
                | ContentContext::Mermaid { .. }
                | ContentContext::MathBlock { .. } => rsx! {
                    ActionItem {
                        label: t!("context_menu.save_image_as").to_string(),
                        icon: IconName::Download,
                        action: Action::FileSaveImageAs,
                        on_close,
                    }
                },
                _ => rsx! {},
            }

            // === Section 5: Lenses ===
            if has_lenses {
                ContextMenuSeparator {}
                LensItems { on_close: on_close }
            }
        }
    }
}

/// A row that runs `action` as its key binding would, showing that binding.
#[component]
fn ActionItem(
    #[props(into)] label: String,
    icon: IconName,
    action: Action,
    on_close: EventHandler<()>,
) -> Element {
    let state = use_context::<crate::state::AppState>();
    rsx! {
        ContextMenuItem {
            label,
            shortcut: shortcut_hint_for_context_action(KeyContext::Content, &action.to_string()),
            icon: Some(icon),
            on_click: move |_| {
                dispatch_action(&action, state);
                on_close.call(());
            },
        }
    }
}

/// A row that puts `text` on the clipboard and says so.
#[component]
fn CopyItem(
    #[props(into)] label: String,
    shortcut: Option<String>,
    text: String,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        ContextMenuItem {
            label,
            shortcut,
            icon: Some(IconName::Copy),
            on_click: move |_| {
                crate::utils::clipboard::copy_text(&text);
                crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
                on_close.call(());
            },
        }
    }
}

/// Copy a Mermaid or Math block as an image using saved element references.
fn copy_special_block_image(is_mermaid: bool, opaque: bool) {
    let kind = if is_mermaid {
        "mermaidBlock"
    } else {
        "mathBlock"
    };
    let opaque_str = if opaque { "true" } else { "false" };
    spawn_detached(async move {
        let js = format!(
            "(async () => {{ dioxus.send(await window.Arto.rasterize.{kind}({opaque_str})); }})()"
        );
        crate::keybindings::dispatcher::copy_rasterized_image(document::eval(&js), kind).await;
    });
}

/// Copy markdown source from file using pre-captured line range and selected text.
fn copy_markdown_source_direct(
    file: std::path::PathBuf,
    source_line: u32,
    source_line_end: u32,
    selected_text: String,
) {
    spawn_detached(async move {
        let extracted = tokio::task::spawn_blocking(move || {
            markdown_to_copy(&file, source_line, source_line_end, &selected_text)
        })
        .await;
        match extracted {
            Ok(Some(md)) => {
                crate::utils::clipboard::copy_text(&md);
                crate::keybindings::dispatcher::show_action_feedback(&t!("context_menu.copied"));
            }
            Ok(None) => {
                tracing::debug!(%source_line, %source_line_end, "No source lines extracted")
            }
            Err(error) => tracing::debug!(%error, "Source extraction task failed"),
        }
    });
}

/// The Markdown behind `selected_text` within lines `start..=end` of
/// `file`, or the lines whole when nothing is selected.
fn markdown_to_copy(file: &Path, start: u32, end: u32, selected_text: &str) -> Option<String> {
    let source = crate::utils::source_lines::extract_source_lines(file, start, end)?;
    if selected_text.trim().is_empty() {
        return Some(source);
    }
    Some(crate::markdown::extract_source_selection(&source, selected_text).unwrap_or(source))
}

/// The Copy Path row: the narrowest of path, path with line and path with
/// range that what was clicked on allows.
#[derive(Debug, PartialEq)]
struct CopyPathRow {
    label: String,
    value: String,
    /// The action whose shortcut the row shows.
    action: &'static str,
}

fn copy_path_row(
    file: Option<&Path>,
    source_line: Option<u32>,
    source_line_end: Option<u32>,
) -> Option<CopyPathRow> {
    let file = file?;
    let row = match (source_line, source_line_end) {
        (Some(start), Some(end)) if start != end => CopyPathRow {
            label: t!(
                "context_menu.copy_path_with_range",
                start = start,
                end = end
            )
            .into_owned(),
            value: format_location(file, start, end),
            action: "clipboard.copy_file_path_with_range",
        },
        (Some(line), _) => CopyPathRow {
            label: t!("context_menu.copy_path_with_line", line = line).into_owned(),
            value: format_location(file, line, line),
            action: "clipboard.copy_file_path_with_line",
        },
        (None, _) => CopyPathRow {
            label: t!("context_menu.copy_path").into_owned(),
            value: file.display().to_string(),
            action: "clipboard.copy_file_path",
        },
    };
    Some(row)
}

fn exec_edit_command(command: &'static str) {
    spawn_detached(async move {
        let js = format!("document.execCommand('{command}');");
        let _ = document::eval(&js).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &str, value: &str, action: &'static str) -> Option<CopyPathRow> {
        Some(CopyPathRow {
            label: label.to_string(),
            value: value.to_string(),
            action,
        })
    }

    #[test]
    fn without_a_selection_the_whole_lines_are_copied() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("doc.md");
        std::fs::write(
            &file,
            indoc::indoc! {"
            # Title

            First paragraph.
            Second line.
        "},
        )
        .unwrap();

        assert_eq!(
            markdown_to_copy(&file, 3, 4, "  "),
            Some("First paragraph.\nSecond line.".to_string())
        );
    }

    #[test]
    fn lines_the_file_does_not_have_copy_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("doc.md");
        std::fs::write(&file, "one line\n").unwrap();

        assert_eq!(markdown_to_copy(&file, 5, 6, ""), None);
        assert_eq!(
            markdown_to_copy(&dir.path().join("gone.md"), 1, 1, ""),
            None
        );
    }

    #[test]
    fn without_a_file_there_is_no_path_to_copy() {
        assert_eq!(copy_path_row(None, Some(3), Some(5)), None);
    }

    #[test]
    fn a_range_of_lines_is_copied_with_the_path() {
        assert_eq!(
            copy_path_row(Some(Path::new("/a/b.md")), Some(3), Some(5)),
            row(
                "Copy Path with Range (3-5)",
                "/a/b.md:3-5",
                "clipboard.copy_file_path_with_range"
            )
        );
    }

    #[test]
    fn a_single_line_is_copied_with_the_path() {
        let expected = row(
            "Copy Path with Line (3)",
            "/a/b.md:3",
            "clipboard.copy_file_path_with_line",
        );
        assert_eq!(
            copy_path_row(Some(Path::new("/a/b.md")), Some(3), Some(3)),
            expected
        );
        assert_eq!(
            copy_path_row(Some(Path::new("/a/b.md")), Some(3), None),
            expected
        );
    }

    #[test]
    fn without_a_line_the_path_alone_is_copied() {
        let expected = row("Copy Path", "/a/b.md", "clipboard.copy_file_path");
        assert_eq!(
            copy_path_row(Some(Path::new("/a/b.md")), None, None),
            expected
        );
        assert_eq!(
            copy_path_row(Some(Path::new("/a/b.md")), None, Some(5)),
            expected
        );
    }
}
