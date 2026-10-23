use dioxus::prelude::*;

/// Call `on_dismiss` when the pointer goes down outside every element that
/// matches `selector`, or when Escape is pressed anywhere.
///
/// Pointer events are watched rather than focus, so a click on a button
/// inside the element never dismisses it on platforms where buttons do not
/// take focus. The listeners are removed when the component unmounts.
pub fn use_dismiss_on_outside(selector: &'static str, mut on_dismiss: impl FnMut() + 'static) {
    let eval = use_hook(|| {
        let eval = document::eval(&dismiss_script(selector));
        let mut receiver = eval;
        spawn(async move {
            while receiver.recv::<serde_json::Value>().await.is_ok() {
                on_dismiss();
            }
        });
        eval
    });
    use_drop(move || {
        let _ = eval.send(true);
    });
}

/// The listeners also remove themselves once nothing matches `selector`, in
/// case the unmount message never reaches a script whose channel is already
/// gone.
fn dismiss_script(selector: &str) -> String {
    let selector = serde_json::to_string(selector).expect("a string serializes");
    format!(
        r#"
        const selector = {selector};
        const controller = new AbortController();
        const options = {{ capture: true, signal: controller.signal }};
        const gone = () => {{
            if (document.querySelector(selector)) return false;
            controller.abort();
            return true;
        }};
        document.addEventListener('pointerdown', (e) => {{
            if (gone()) return;
            const inside = e.target instanceof Element && e.target.closest(selector);
            if (!inside) dioxus.send(true);
        }}, options);
        document.addEventListener('keydown', (e) => {{
            if (gone()) return;
            if (e.key === 'Escape') dioxus.send(true);
        }}, options);
        await dioxus.recv();
        controller.abort();
        "#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selector_is_written_into_the_script_as_a_string_literal() {
        let script = dismiss_script(r#".form[data-x="1"]"#);
        assert!(
            script.contains(r#"const selector = ".form[data-x=\"1\"]";"#),
            "{script}"
        );
    }
}
