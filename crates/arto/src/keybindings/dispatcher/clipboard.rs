//! Putting what is on screen somewhere else: the clipboard, and a file.
//!
//! Every one of these reaches into the page, because what is being copied is
//! whatever the content cursor is on and only the page knows what that is.

use dioxus::document;

use super::*;

pub(super) fn copy_content_cursor_text(js_getter: &'static str) {
    spawn_detached(async move {
        let js = format!(
            "(() => {{ const t = window.Arto?.contentCursor?.{js_getter}() ?? ''; dioxus.send(t); }})()"
        );
        let mut eval = document::eval(&js);
        match eval.recv::<String>().await {
            Ok(text) if !text.is_empty() => {
                // What the document shows for an image is a URL only this app
                // can resolve, so anything leaving it says where the file is.
                let text = crate::assets::images::with_paths_for_urls(&text);
                crate::utils::clipboard::copy_text(&text);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(js_getter, "Content cursor copy failed: {e}"),
        }
    });
}

pub(super) fn copy_image_from_cursor(opaque: bool) {
    spawn_detached(async move {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum CopyImageTarget {
            Image { src: String },
            Math,
            Mermaid,
            None,
        }

        let js = r#"
            (() => {
                const cursor = window.Arto?.contentCursor;
                const el = cursor?.getCurrentElement?.();
                if (!el) { dioxus.send({ kind: 'none' }); return; }

                if (el.tagName === 'IMG') {
                    const src = cursor?.getImageSrc?.() ?? '';
                    if (!src) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'image', src });
                    return;
                }

                if (
                    el instanceof HTMLElement &&
                    (
                        el.classList.contains('preprocessed-math-display') ||
                        el.classList.contains('preprocessed-math')
                    )
                ) {
                    dioxus.send({ kind: 'math' });
                    return;
                }

                if (el instanceof HTMLElement && el.classList.contains('preprocessed-mermaid')) {
                    dioxus.send({ kind: 'mermaid' });
                    return;
                }

                dioxus.send({ kind: 'none' });
            })();
        "#;
        let mut eval = document::eval(js);
        let Ok(target) = eval.recv::<CopyImageTarget>().await else {
            return;
        };

        match target {
            CopyImageTarget::Image { src } => {
                copy_image_from_src(src, opaque).await;
            }
            CopyImageTarget::Math => {
                copy_special_block_from_cursor("mathElement", opaque).await;
            }
            CopyImageTarget::Mermaid => {
                copy_special_block_from_cursor("mermaidElement", opaque).await;
            }
            CopyImageTarget::None => {}
        }
    });
}

/// Put the PNG a rasterization returns on the clipboard, and say so when
/// there is none.
///
/// `subject` names what was being rasterized, for the log line. Every way
/// this can fail is reported: silence is what let a Copy Image that copied
/// nothing look like a menu item nobody had clicked.
pub(crate) async fn copy_rasterized_image(mut eval: document::Eval, subject: &str) {
    match eval.recv::<Option<String>>().await {
        Ok(Some(data_url)) => {
            crate::utils::clipboard::copy_image_from_data_url(&data_url);
            show_action_feedback("Copied");
        }
        Ok(None) => {
            tracing::warn!(%subject, "Rasterizing for clipboard copy produced no image")
        }
        Err(e) => {
            tracing::warn!(%e, %subject, "Rasterizing for clipboard copy failed")
        }
    }
}

pub(super) async fn copy_special_block_from_cursor(kind: &str, opaque: bool) {
    let opaque_str = if opaque { "true" } else { "false" };
    let js = format!(
        r#"
        (async () => {{
            const cursor = window.Arto?.contentCursor;
            const el = cursor?.getCurrentElement?.();
            if (!(el instanceof HTMLElement)) {{ dioxus.send(null); return; }}
            dioxus.send(await window.Arto.rasterize.{kind}(el, {opaque_str}));
        }})();
        "#,
    );
    copy_rasterized_image(document::eval(&js), kind).await;
}

pub(crate) async fn copy_image_from_src(src: String, opaque: bool) {
    // An image the app serves for the document is read here rather than in the
    // WebView. Drawing it to a canvas there would taint the canvas — it comes
    // from the app's own origin, not the document's — and asking for it as a
    // CORS request is worse still, because a custom scheme does not take part
    // in CORS and the load simply fails. Reading it costs one encode of an
    // image somebody deliberately asked to copy.
    let rasterize_src = if let Some(data_url) = crate::assets::images::data_url_for(&src) {
        data_url
    } else if src.starts_with("http://") || src.starts_with("https://") {
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn({
            let src = src.clone();
            move || {
                let _ = tx.send(crate::utils::image::download_image_as_data_url(&src));
            }
        });
        match rx.await {
            Ok(Ok(data_url)) => data_url,
            Ok(Err(e)) => {
                tracing::error!(%e, "Failed to download image for clipboard copy");
                return;
            }
            Err(_) => {
                tracing::error!("Image download thread was cancelled");
                return;
            }
        }
    } else {
        src
    };

    let Ok(src_json) = serde_json::to_string(&rasterize_src) else {
        tracing::error!("Failed to serialize image src as JSON");
        return;
    };
    let opaque_str = if opaque { "true" } else { "false" };
    let js = format!(
        "(async () => {{ dioxus.send(await window.Arto.rasterize.image({}, {})); }})();",
        src_json, opaque_str
    );
    copy_rasterized_image(document::eval(&js), "image").await;
}

pub(super) fn copy_image_path_from_cursor() {
    spawn_detached(async move {
        let js =
            "(() => { const src = window.Arto?.contentCursor?.getImageSrc?.() ?? ''; dioxus.send(src); })()";
        let mut eval = document::eval(js);
        match eval.recv::<String>().await {
            Ok(src) if !src.is_empty() => {
                // The path the reader means, not the URL the WebView was given.
                let src = crate::assets::images::with_paths_for_urls(&src);
                crate::utils::clipboard::copy_text(&src);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("Copy image path failed: {e}"),
        }
    });
}

pub(super) fn copy_link_path_from_cursor() {
    spawn_detached(async move {
        let js =
            "(() => { const href = window.Arto?.contentCursor?.getLinkHref?.() ?? ''; dioxus.send(href); })()";
        let mut eval = document::eval(js);
        match eval.recv::<String>().await {
            Ok(href) if !href.is_empty() => {
                crate::utils::clipboard::copy_text(href);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("Copy link path failed: {e}"),
        }
    });
}

pub(super) fn copy_file_path_with_line(file: std::path::PathBuf, is_range: bool) {
    spawn_detached(async move {
        let js =
            "(() => { dioxus.send(window.Arto?.contentCursor?.getSourceLineRange() ?? null); })()";
        let mut eval = document::eval(js);
        if let Ok(Some((start, end))) = eval.recv::<Option<(u32, u32)>>().await {
            let path_str = file.display().to_string();
            let text = if is_range && start != end {
                format!("{path_str}:{start}-{end}")
            } else {
                format!("{path_str}:{start}")
            };
            crate::utils::clipboard::copy_text(&text);
            show_action_feedback("Copied");
        }
    });
}

pub(super) fn copy_markdown_source(file: std::path::PathBuf) {
    spawn_detached(async move {
        #[derive(serde::Deserialize)]
        struct MarkdownSourceRequest {
            range: Option<(u32, u32)>,
            selected_text: String,
        }

        let js = r#"
            (() => {
                const range = window.Arto?.contentCursor?.getSourceLineRange?.() ?? null;
                const selection = window.getSelection();
                const selected_text = selection ? selection.toString() : "";
                dioxus.send({ range, selected_text });
            })()
        "#;
        let mut eval = document::eval(js);
        if let Ok(MarkdownSourceRequest {
            range: Some((start, end)),
            selected_text,
        }) = eval.recv::<MarkdownSourceRequest>().await
        {
            let handle = std::thread::spawn(move || {
                let source = crate::utils::source_lines::extract_source_lines(&file, start, end)?;
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
                    show_action_feedback("Copied");
                }
                Ok(None) => tracing::debug!(%start, %end, "No source lines extracted"),
                Err(_) => tracing::debug!("Source extraction thread panicked"),
            }
        }
    });
}

pub(super) fn save_image_from_cursor() {
    spawn_detached(async move {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum SaveImageTarget {
            Image { src: String },
            Math,
            Mermaid,
            None,
        }

        let js = r#"
            (() => {
                const cursor = window.Arto?.contentCursor;
                const el = cursor?.getCurrentElement?.();
                if (!el) { dioxus.send({ kind: 'none' }); return; }

                if (el.tagName === 'IMG') {
                    const src = cursor?.getImageSrc?.() ?? '';
                    if (!src) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'image', src });
                    return;
                }

                if (
                    el instanceof HTMLElement &&
                    (
                        el.classList.contains('preprocessed-math-display') ||
                        el.classList.contains('preprocessed-math')
                    )
                ) {
                    dioxus.send({ kind: 'math' });
                    return;
                }

                if (el instanceof HTMLElement && el.classList.contains('preprocessed-mermaid')) {
                    dioxus.send({ kind: 'mermaid' });
                    return;
                }

                dioxus.send({ kind: 'none' });
            })()
        "#;
        let mut eval = document::eval(js);
        let Ok(target) = eval.recv::<SaveImageTarget>().await else {
            return;
        };

        match target {
            SaveImageTarget::Image { src } => {
                std::thread::spawn(move || {
                    // `save_image` knows `data:` and `http(s)`; the app's own
                    // protocol resolves nowhere outside a WebView, so an image
                    // the app serves is read here and handed over as bytes.
                    let src = crate::assets::images::data_url_for(&src).unwrap_or(src);
                    crate::utils::image::save_image(&src);
                });
            }
            SaveImageTarget::Math => {
                save_special_block_from_cursor("mathElement").await;
            }
            SaveImageTarget::Mermaid => {
                save_special_block_from_cursor("mermaidElement").await;
            }
            SaveImageTarget::None => {}
        }
    });
}

pub(super) async fn save_special_block_from_cursor(kind: &str) {
    let js = format!(
        r#"
        (async () => {{
            const cursor = window.Arto?.contentCursor;
            const el = cursor?.getCurrentElement?.();
            if (!(el instanceof HTMLElement)) {{ dioxus.send(null); return; }}
            dioxus.send(await window.Arto.rasterize.{kind}(el, true));
        }})();
        "#,
    );
    let mut eval = document::eval(&js);
    match eval.recv::<Option<String>>().await {
        Ok(Some(data_url)) => {
            std::thread::spawn(move || {
                crate::utils::image::save_image(&data_url);
            });
        }
        Ok(None) => tracing::warn!(%kind, "Rasterizing for save produced no image"),
        Err(e) => tracing::warn!(%e, %kind, "Rasterizing for save failed"),
    }
}
