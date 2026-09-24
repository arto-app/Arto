//! The `codex` command-line agent, asked through its app server, which
//! streams the answer as JSON-RPC notifications.
//!
//! Not `codex exec`: it reports the answer only once it is whole. The
//! sandbox, the ephemeral thread and the system prompt are asked for when
//! the thread starts (see [`crate::lenses::app_server`]).

use super::{Answer, Conversation, ProgramAgent, Reader};
use serde_json::{json, Value};

pub(super) struct Codex;

impl ProgramAgent for Codex {
    fn args(&self, model: Option<&str>) -> Vec<String> {
        let mut args: Vec<String> = [
            "app-server",
            // A read-only sandbox still lets the agent run commands that
            // read files, and the document it is handed can ask it to — to
            // read what lies beside the document and write it into the
            // answer. A lens hands over text to be read, so every tool is
            // turned off, and so is what the user's configuration could
            // bring in besides.
            "--disable",
            "plugins",
            "--disable",
            "hooks",
            "--disable",
            "shell_tool",
            "--disable",
            "unified_exec",
            "--disable",
            "apps",
            "--disable",
            "browser_use",
            "--disable",
            "computer_use",
            "--disable",
            "image_generation",
            "-c",
            "web_search=\"disabled\"",
        ]
        .map(String::from)
        .into();
        if let Some(model) = model {
            // The value is read as TOML, whose basic strings JSON's are.
            args.extend(["-c".to_string(), format!("model={}", json!(model))]);
        }
        args
    }

    fn conversation(&self) -> Conversation {
        Conversation::AppServer
    }

    fn models_args(&self) -> &'static [&'static str] {
        &["debug", "models"]
    }
}

impl Reader for Codex {
    fn read(&self, message: &Value, answer: &mut Answer) {
        let params = &message["params"];
        let failure = |error: &Value| {
            error["message"]
                .as_str()
                .unwrap_or("codex failed")
                .to_string()
        };
        match message["method"].as_str() {
            Some("item/agentMessage/delta") => answer.push_to(
                params["itemId"].as_str().unwrap_or_default(),
                params["delta"].as_str().unwrap_or_default(),
            ),
            Some("item/completed") if params["item"]["type"] == "agentMessage" => {
                if let Some(text) = params["item"]["text"].as_str() {
                    answer.report(text);
                }
            }
            Some("turn/completed") if params["turn"]["status"] == "failed" => {
                answer.fail(failure(&params["turn"]["error"]));
            }
            Some("error") if params["willRetry"] != true => {
                answer.fail(failure(&params["error"]));
            }
            // A request the server refused.
            None if message.get("error").is_some() => answer.fail(failure(&message["error"])),
            _ => {}
        }
    }

    /// The models `codex debug models` lists for choosing: its catalog
    /// holds internal ones too, which it marks as hidden.
    fn models(&self, catalog: &str) -> Result<Vec<String>, String> {
        let catalog: Value = serde_json::from_str(catalog).map_err(|error| error.to_string())?;
        Ok(catalog["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|model| model["visibility"].as_str() != Some("hide"))
            .filter_map(|model| model["slug"].as_str().map(str::to_string))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::{invocation, Decoder, Transport};
    use super::*;
    use arto_config::LensAgent;

    #[test]
    fn codex_is_asked_through_its_app_server_with_its_tools_off() {
        let run = invocation(&lens(Some(LensAgent::Codex), None));
        assert!(
            matches!(run.transport, Transport::AppServer { .. }),
            "{:?}",
            run.transport
        );
        let argv = argv(&run);
        assert_eq!(argv[1], "app-server");
        let disabled: Vec<&str> = argv
            .windows(2)
            .filter(|pair| pair[0] == "--disable")
            .map(|pair| pair[1].as_str())
            .collect();
        for tool in ["shell_tool", "unified_exec", "plugins", "hooks"] {
            assert!(disabled.contains(&tool), "{tool} is left on: {argv:?}");
        }
        assert!(!argv.iter().any(|arg| arg.starts_with("model=")));
    }

    #[test]
    fn codex_is_given_the_model_as_a_toml_string() {
        let run = invocation(&lens(Some(LensAgent::Codex), Some("gpt-5.5")));
        let argv = argv(&run);
        assert_eq!(&argv[argv.len() - 2..], ["-c", r#"model="gpt-5.5""#]);
    }

    /// What `codex app-server` wrote for a short answer, cut to the messages
    /// that matter and a few that do not.
    const APP_SERVER: &str = concat!(
        r#"{"id":1,"result":{"userAgent":"codex"}}"#,
        "\n",
        r#"{"method":"thread/started","params":{"thread":{"id":"t"}}}"#,
        "\n",
        r#"{"method":"item/started","params":{"item":{"type":"reasoning","id":"rs_1"}}}"#,
        "\n",
        r##"{"method":"item/agentMessage/delta","params":{"threadId":"t","turnId":"u","itemId":"msg_1","delta":"# 見出"}}"##,
        "\n",
        r#"{"method":"item/agentMessage/delta","params":{"threadId":"t","turnId":"u","itemId":"msg_1","delta":"し\n\n本文。"}}"#,
        "\n",
        r##"{"method":"item/completed","params":{"item":{"type":"agentMessage","id":"msg_1","text":"# 見出し\n\n本文。","phase":"final_answer"}}}"##,
        "\n",
        r#"{"method":"turn/completed","params":{"threadId":"t","turn":{"id":"u","status":"completed","error":null}}}"#,
        "\n",
    );

    #[test]
    fn codex_answers_as_its_deltas_arrive() {
        let mut decoder = Decoder::new(Some(LensAgent::Codex));
        let line_end = |n: usize| APP_SERVER.match_indices('\n').nth(n).unwrap().0 + 1;
        assert_eq!(decoder.feed(&APP_SERVER[..line_end(2)]), "");
        assert_eq!(decoder.feed(&APP_SERVER[..line_end(3)]), "# 見出");
        assert_eq!(
            decoder.finish(APP_SERVER).as_deref(),
            Ok("# 見出し\n\n本文。")
        );
    }

    #[test]
    fn codex_answers_with_its_last_message_only() {
        let output = concat!(
            r#"{"method":"item/agentMessage/delta","params":{"itemId":"a","delta":"Let me look."}}"#,
            "\n",
            r#"{"method":"item/agentMessage/delta","params":{"itemId":"b","delta":"Answer"}}"#,
            "\n",
        );
        assert_eq!(answer(LensAgent::Codex, output).as_deref(), Ok("Answer"));
    }

    #[test]
    fn codex_reports_a_failed_turn_but_not_an_error_it_retries() {
        let output = concat!(
            r#"{"method":"error","params":{"error":{"message":"reconnecting"},"willRetry":true}}"#,
            "\n",
            r#"{"method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"stream disconnected"}}}}"#,
            "\n",
        );
        assert_eq!(
            answer(LensAgent::Codex, output),
            Err("stream disconnected".to_string())
        );
    }

    #[test]
    fn codex_reports_a_request_it_refused() {
        let output = r#"{"id":3,"error":{"code":-32600,"message":"unknown model"}}"#;
        assert_eq!(
            answer(LensAgent::Codex, output),
            Err("unknown model".to_string())
        );
    }

    #[test]
    fn codex_offers_what_its_catalog_does_not_hide() {
        let catalog = r#"{"models": [
            {"slug": "gpt-reserve", "visibility": "hide"},
            {"slug": "gpt-5.6-sol", "visibility": "list"},
            {"slug": "gpt-5.5", "visibility": "list"}
        ]}"#;

        assert_eq!(Codex.models(catalog).unwrap(), ["gpt-5.6-sol", "gpt-5.5"]);
    }
}
