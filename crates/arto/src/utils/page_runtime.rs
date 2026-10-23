/// A script that runs `body` once the page's runtime has installed `member`
/// of `window.Arto` (`"search.setup"` for `window.Arto.search.setup`).
///
/// The runtime is imported asynchronously, so a hook that subscribes to the
/// page can run before there is anything to subscribe to; the script polls
/// rather than being lost.
pub fn once_arto_has(member: &str, body: &str) -> String {
    let guard = member
        .split('.')
        .fold("window.Arto".to_string(), |path, part| {
            format!("{path}?.{part}")
        });
    let body = body
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    indoc::formatdoc! {r#"
        (async () => {{
            while (!{guard}) {{
                await new Promise(resolve => setTimeout(resolve, 10));
            }}
        {body}
        }})();
    "#}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_waits_for_the_member_before_running_the_body() {
        assert_eq!(
            once_arto_has("search.setup", "window.Arto.search.setup(dioxus.send);"),
            indoc::indoc! {r#"
                (async () => {
                    while (!window.Arto?.search?.setup) {
                        await new Promise(resolve => setTimeout(resolve, 10));
                    }
                    window.Arto.search.setup(dioxus.send);
                })();
            "#}
        );
    }
}
