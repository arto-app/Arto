//! The find field's actions.
//!
//! The field itself is `crate::components::find`; these are what the
//! bindings in the `search` context do to it.

use dioxus::document;

use super::*;

pub(super) fn search_navigate_eval(direction: &'static str) {
    spawn_detached(async move {
        let js = format!("window.Arto.search.navigate('{direction}')");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%direction, "Search navigate failed: {e}");
        }
    });
}

pub(super) fn search_open(state: &mut AppState) {
    let mut app_state = *state;
    spawn_detached(async move {
        let js = r#"
            (() => {
                const s = window.getSelection();
                dioxus.send(s ? s.toString() : "");
            })()
        "#;
        let mut eval = document::eval(js);
        match eval.recv::<String>().await {
            Ok(text) if !text.trim().is_empty() => {
                app_state.open_search_with_text(Some(text));
            }
            Ok(_) | Err(_) => {
                app_state.open_search_with_text(None);
            }
        }
    });
}

pub(super) fn search_pin_current(state: &mut AppState) {
    let mut app_state = *state;
    spawn_detached(async move {
        #[derive(serde::Deserialize)]
        struct QueryValue {
            value: String,
        }

        let mut eval = document::eval(
            r#"
            (() => {
                const input = document.querySelector('.search-input');
                dioxus.send({ value: input?.value || '' });
            })()
            "#,
        );
        match eval.recv::<QueryValue>().await {
            Ok(result) if !result.value.is_empty() => {
                let _ = add_pinned_search(result.value);
                app_state.update_search_results(0, 0);
                let _ = document::eval(
                    r#"
                    (() => {
                        const input = document.querySelector('.search-input');
                        if (input) {
                            input.value = '';
                            input.focus();
                        }
                        window.Arto.search.clear();
                    })()
                    "#,
                )
                .await;
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("Search pin current failed: {e}"),
        }
    });
}
