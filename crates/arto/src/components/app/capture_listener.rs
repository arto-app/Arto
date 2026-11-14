use arto_lsp::{CaptureResult, ErrorCode, LineRange};
use dioxus::desktop::window;
use dioxus::document;
use dioxus::prelude::*;
use serde::Deserialize;

use crate::events::{CaptureTarget, CAPTURE_IN_WINDOW};

/// Answer `arto/capture` for this window.
///
/// The page draws the picture itself, so this is a round trip into the
/// WebView, like the selection — and `document::eval` only reaches it from
/// the window's own scope.
pub(super) fn setup_capture_listener() {
    use_future(move || async move {
        let window_id = window().id();
        let mut rx = CAPTURE_IN_WINDOW.subscribe();

        loop {
            match rx.recv().await {
                Ok((target_window, token, target)) if target_window == window_id => {
                    // Bounded, because this loop cannot take the next
                    // request until this one is answered.
                    match tokio::time::timeout(CAPTURE_TIMEOUT, draw(&target)).await {
                        Ok(Ok(Some(picture))) => {
                            crate::ipc::answer_parked(token, |window| CaptureResult {
                                window,
                                png: picture.png,
                                width: picture.width,
                                height: picture.height,
                                lines: picture.lines,
                            })
                        }
                        Ok(Ok(None)) => crate::ipc::refuse_parked(
                            token,
                            ErrorCode::InvalidParams,
                            &nothing_at(&target),
                        ),
                        Ok(Err(error)) => crate::ipc::abandon_parked(token, &error),
                        Err(_) => {
                            crate::ipc::abandon_parked(token, "the window did not finish drawing");
                        }
                    }
                }
                Ok(_) => {}
                // The client is released by the server's own deadline, so a
                // dropped request costs a wait rather than an answer.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::debug!(?window_id, missed, "Missed capture requests");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// A picture as the page's `window.Arto.capture` hands it back.
#[derive(Debug, Deserialize, PartialEq)]
struct PagePicture {
    png: String,
    width: u32,
    height: u32,
    #[serde(default)]
    lines: Option<LineRange>,
}

/// The picture, `None` when the page found nothing to draw, or why the page
/// could not answer at all.
async fn draw(target: &CaptureTarget) -> Result<Option<PagePicture>, String> {
    let mut eval = document::eval(&capture_js(target));
    let answer = eval
        .recv::<serde_json::Value>()
        .await
        .map_err(|error| format!("the page did not answer: {error}"))?;
    read_answer(answer)
}

fn read_answer(answer: serde_json::Value) -> Result<Option<PagePicture>, String> {
    if answer.is_null() {
        return Ok(None);
    }
    if let Some(error) = answer.get("error").and_then(|error| error.as_str()) {
        return Err(format!("the page could not draw it: {error}"));
    }
    serde_json::from_value(answer)
        .map(Some)
        .map_err(|error| format!("the page answered with something unreadable: {error}"))
}

fn capture_js(target: &CaptureTarget) -> String {
    let call = match target {
        CaptureTarget::Block(line) => format!("window.Arto.capture.line({line})"),
        CaptureTarget::Heading(id) => format!(
            "window.Arto.capture.heading({})",
            serde_json::to_string(id).unwrap_or_else(|_| "\"\"".to_string())
        ),
        CaptureTarget::Viewport => "window.Arto.capture.viewport()".to_string(),
    };
    // A rejection is sent as an error rather than left to the eval, which
    // would only ever report that no answer came.
    format!(
        "(async () => {{ try {{ dioxus.send((await {call}) ?? null); }} \
         catch (e) {{ dioxus.send({{ error: String(e) }}); }} }})()"
    )
}

fn nothing_at(target: &CaptureTarget) -> String {
    match target {
        CaptureTarget::Block(line) => format!("no block at line {line}"),
        CaptureTarget::Heading(id) => format!("no heading with the id {id:?}"),
        CaptureTarget::Viewport => "no document to draw".to_string(),
    }
}

/// Inside the server's own deadline for a drawing, so a page that does not
/// finish is reported as such rather than as the app hanging.
const CAPTURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_target_asks_the_page_s_own_capture() {
        assert!(capture_js(&CaptureTarget::Block(42)).contains("window.Arto.capture.line(42)"));
        assert!(capture_js(&CaptureTarget::Viewport).contains("window.Arto.capture.viewport()"));
        assert!(capture_js(&CaptureTarget::Heading("install".into()))
            .contains(r#"window.Arto.capture.heading("install")"#));
    }

    #[test]
    fn a_heading_id_is_encoded_rather_than_interpolated() {
        let js = capture_js(&CaptureTarget::Heading(r#"a"b\c"#.into()));
        assert!(js.contains(r#""a\"b\\c""#), "{js}");
    }

    #[test]
    fn the_capture_stays_inside_the_server_s_deadline() {
        assert!(CAPTURE_TIMEOUT < arto_lsp::READY_TIMEOUT);
    }

    #[test]
    fn a_picture_is_read_with_its_lines() {
        let answer =
            json!({"png": "UE5H", "width": 4, "height": 3, "lines": {"start": 2, "end": 5}});
        assert_eq!(
            read_answer(answer),
            Ok(Some(PagePicture {
                png: "UE5H".into(),
                width: 4,
                height: 3,
                lines: Some(LineRange { start: 2, end: 5 }),
            }))
        );
    }

    #[test]
    fn nothing_to_draw_is_not_a_failure() {
        assert_eq!(read_answer(serde_json::Value::Null), Ok(None));
    }

    #[test]
    fn a_page_that_threw_says_what_it_threw() {
        let error = read_answer(json!({"error": "Error: canvas too large"})).unwrap_err();
        assert!(error.contains("canvas too large"), "{error}");
    }
}
