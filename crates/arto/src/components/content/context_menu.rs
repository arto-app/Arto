mod copy_as;
mod copy_code_as;
mod copy_path_as;
mod copy_table_as;
mod data;
mod image_ops;
mod source_ops;

pub use data::*;

use dioxus::document;
use dioxus::prelude::*;
use std::path::PathBuf;

use crate::components::context_menu::{ContextMenuItem, ContextMenuSeparator};
use crate::components::icon::IconName;
use crate::keybindings::dispatcher::dispatch_action;
use crate::keybindings::{shortcut_hint_for_context_action, Action, KeyContext};
use copy_as::CopyAsSubmenu;
use copy_code_as::CopyCodeAsSubmenu;
use copy_path_as::CopyPathAsSubmenu;
use copy_table_as::CopyTableAsSubmenu;
use image_ops::{CopyImageAsSubmenu, CopySpecialBlockAsSubmenu};
use source_ops::LinkContextItems;

#[component]
pub fn ContentContextMenu(
    position: (i32, i32),
    context: ContentContext,
    has_selection: bool,
    selected_text: String,
    current_file: Option<PathBuf>,
    base_dir: PathBuf,
    source_line: Option<u32>,
    source_line_end: Option<u32>,
    table_csv: Option<String>,
    table_tsv: Option<String>,
    table_markdown: Option<String>,
    table_source_line: Option<u32>,
    table_source_line_end: Option<u32>,
    on_close: EventHandler<()>,
) -> Element {
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
    let has_any_submenu = has_selection
        || has_file
        || copy_code_source.is_some()
        || has_table
        || is_image
        || is_special_block;

    // Determine smart default for Copy Path label and value
    let (copy_path_label, copy_path_value) =
        match (current_file.as_ref(), source_line, source_line_end) {
            (Some(f), Some(start), Some(end)) if start != end => {
                let path_str = f.display().to_string();
                (
                    format!("Copy Path with Range ({start}-{end})"),
                    Some(format!("{path_str}:{start}-{end}")),
                )
            }
            (Some(f), Some(line), _) => {
                let path_str = f.display().to_string();
                (
                    format!("Copy Path with Line ({line})"),
                    Some(format!("{path_str}:{line}")),
                )
            }
            (Some(f), None, _) => {
                let path_str = f.display().to_string();
                ("Copy Path".to_string(), Some(path_str))
            }
            (None, _, _) => ("Copy Path".to_string(), None),
        };

    rsx! {
        // Backdrop to close menu on outside click
        div {
            class: "context-menu-backdrop",
            // Prevent mousedown from clearing text selection
            onmousedown: move |evt| evt.prevent_default(),
            onclick: move |_| {
                on_close.call(());
            },
        }

        // Context menu
        div {
            class: "context-menu content-context-menu",
            style: "left: {position.0}px; top: {position.1}px;",
            // Prevent mousedown from clearing text selection
            onmousedown: move |evt| evt.prevent_default(),
            onclick: move |evt| evt.stop_propagation(),

            // === Section 1: Smart default copy operations ===
            if has_selection {
                ContextMenuItem {
                    label: "Copy",
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
                ContextMenuItem {
                    label: "Copy Code",
                    shortcut: shortcut("clipboard.copy_code"),
                    icon: Some(IconName::Copy),
                    on_click: {
                        let on_close = on_close;
                        move |_| {
                            crate::utils::clipboard::copy_text(&code_content);
                            crate::keybindings::dispatcher::show_action_feedback("Copied");
                            on_close.call(());
                        }
                    },
                }
            }

            // Copy Table (smart default: TSV)
            if let Some(tsv) = table_tsv.clone() {
                ContextMenuItem {
                    label: "Copy Table",
                    shortcut: shortcut("clipboard.copy_table_as_tsv"),
                    icon: Some(IconName::Copy),
                    on_click: {
                        let on_close = on_close;
                        move |_| {
                            crate::utils::clipboard::copy_text(&tsv);
                            crate::keybindings::dispatcher::show_action_feedback("Copied");
                            on_close.call(());
                        }
                    },
                }
            }

            // Copy Image (smart default: transparent background)
            if let Some((ref src, _)) = image_info {
                ContextMenuItem {
                    label: "Copy Image",
                    shortcut: shortcut("clipboard.copy_image"),
                    icon: Some(IconName::Photo),
                    on_click: {
                        let on_close = on_close;
                        let src = src.clone();
                        move |_| {
                            let src = src.clone();
                            spawn(async move {
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
                    label: "Copy Image",
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
            if let Some(value) = copy_path_value.clone() {
                ContextMenuItem {
                    label: copy_path_label.clone(),
                    shortcut: shortcut(copy_path_shortcut_action_str(source_line, source_line_end)),
                    icon: Some(IconName::Copy),
                    on_click: {
                        let on_close = on_close;
                        move |_| {
                            crate::utils::clipboard::copy_text(&value);
                            crate::keybindings::dispatcher::show_action_feedback("Copied");
                            on_close.call(());
                        }
                    },
                }
            }

            // === Section 2: Selection and search ===
            ContextMenuSeparator {}

            ContextMenuItem {
                label: "Select All",
                icon: Some(IconName::SelectAll),
                on_click: {
                    let on_close = on_close;
                    move |_| {
                        exec_edit_command("selectAll");
                        on_close.call(());
                    }
                },
            }

            ContextMenuItem {
                label: "Find in Page",
                shortcut: shortcut("search.open"),
                icon: Some(IconName::Search),
                on_click: {
                    let on_close = on_close;
                    move |_| {
                        dispatch_action(&Action::SearchOpen, state);
                        on_close.call(());
                    }
                },
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
                ContentContext::Image { .. } => rsx! {
                    ContextMenuItem {
                        label: "Save Image As...",
                        shortcut: shortcut("file.save_image_as"),
                        icon: Some(IconName::Download),
                        on_click: {
                            let on_close = on_close;
                            move |_| {
                                dispatch_action(&Action::FileSaveImageAs, state);
                                on_close.call(());
                            }
                        },
                    }
                },
                ContentContext::Mermaid { .. } | ContentContext::MathBlock { .. } => rsx! {
                    ContextMenuItem {
                        label: "Save Image As...",
                        shortcut: shortcut("file.save_image_as"),
                        icon: Some(IconName::Download),
                        on_click: {
                            let on_close = on_close;
                            move |_| {
                                dispatch_action(&Action::FileSaveImageAs, state);
                                on_close.call(());
                            }
                        },
                    }
                },
                _ => rsx! {},
            }
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
    spawn(async move {
        let js = format!(
            "(async () => {{ dioxus.send(await window.Arto.rasterize.{kind}({opaque_str})); }})()"
        );
        let mut eval = document::eval(&js);
        if let Ok(Some(data_url)) = eval.recv::<Option<String>>().await {
            crate::utils::clipboard::copy_image_from_data_url(&data_url);
            crate::keybindings::dispatcher::show_action_feedback("Copied");
        }
    });
}

/// Copy markdown source from file using pre-captured line range and selected text.
fn copy_markdown_source_direct(
    file: std::path::PathBuf,
    source_line: u32,
    source_line_end: u32,
    selected_text: String,
) {
    spawn(async move {
        let handle = std::thread::spawn(move || {
            let source = crate::utils::source_lines::extract_source_lines(
                &file,
                source_line,
                source_line_end,
            )?;
            if selected_text.trim().is_empty() {
                return Some(source);
            }
            Some(
                crate::markdown::extract_source_selection(&source, &selected_text)
                    .unwrap_or(source),
            )
        });
        match handle.join() {
            Ok(Some(md)) => {
                crate::utils::clipboard::copy_text(&md);
                crate::keybindings::dispatcher::show_action_feedback("Copied");
            }
            Ok(None) => {
                tracing::debug!(%source_line, %source_line_end, "No source lines extracted")
            }
            Err(_) => tracing::debug!("Source extraction thread panicked"),
        }
    });
}

fn copy_path_shortcut_action_str(
    source_line: Option<u32>,
    source_line_end: Option<u32>,
) -> &'static str {
    match (source_line, source_line_end) {
        (Some(start), Some(end)) if start != end => "clipboard.copy_file_path_with_range",
        (Some(_), _) => "clipboard.copy_file_path_with_line",
        _ => "clipboard.copy_file_path",
    }
}

fn exec_edit_command(command: &'static str) {
    spawn(async move {
        let js = format!("document.execCommand('{command}');");
        let _ = document::eval(&js).await;
    });
}
