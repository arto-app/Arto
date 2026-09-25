//! The `claude` command-line agent, which streams its answer as
//! `stream-json` events.

use super::{text, Answer, ProgramAgent, Reader, SYSTEM_PROMPT};
use arto_config::LensCapability;
use serde_json::Value;

/// The aliases `claude` takes that its help may not name.
const ALIASES: [&str; 3] = ["opus", "sonnet", "haiku"];

pub(super) struct Claude;

/// The tools that give `capability`.
fn tools(capability: LensCapability) -> &'static [&'static str] {
    match capability {
        LensCapability::WebSearch => &["WebSearch", "WebFetch"],
        LensCapability::ReadFiles => &["Read", "Grep", "Glob"],
        LensCapability::Shell => &["Bash"],
    }
}

impl ProgramAgent for Claude {
    fn args(&self, model: Option<&str>, allow: &[LensCapability]) -> Vec<String> {
        let tools = allow
            .iter()
            .flat_map(|&capability| tools(capability))
            .copied()
            .collect::<Vec<_>>()
            .join(",");
        let mut args: Vec<String> = vec!["-p".to_string(), "--no-session-persistence".to_string()];
        // `--tools` makes them available, `--allowedTools` lets them run
        // without the prompt `-p` has no one to answer.
        args.extend(["--tools".to_string(), tools.clone()]);
        if !tools.is_empty() {
            args.extend(["--allowedTools".to_string(), tools]);
        }
        args.extend(
            [
                "--strict-mcp-config",
                "--setting-sources",
                "",
                "--disable-slash-commands",
                "--system-prompt",
                SYSTEM_PROMPT,
                "--output-format",
                "stream-json",
                "--include-partial-messages",
                "--verbose",
            ]
            .map(String::from),
        );
        if let Some(model) = model {
            args.extend(["--model".to_string(), model.to_string()]);
        }
        args
    }

    fn models_args(&self) -> &'static [&'static str] {
        &["--help"]
    }
}

impl Reader for Claude {
    fn read(&self, event: &Value, answer: &mut Answer) {
        match event["type"].as_str() {
            Some("stream_event") => {
                let delta = &event["event"]["delta"];
                // With tools, the text before a tool call is a message of
                // its own; only the one after the last call is the answer.
                if event["event"]["type"] == "message_start" {
                    answer.start_over();
                } else if event["event"]["type"] == "content_block_delta"
                    && delta["type"] == "text_delta"
                {
                    answer.push(delta["text"].as_str().unwrap_or_default());
                }
            }
            Some("result") => {
                if event["is_error"].as_bool().unwrap_or(false) {
                    if let Some(error) = text(&event["result"]).or_else(|| text(&event["subtype"]))
                    {
                        answer.fail(error);
                    }
                } else if let Some(result) = text(&event["result"]) {
                    answer.report(result);
                }
            }
            _ => {}
        }
    }

    /// `claude` has no list of models; its help names some of the aliases
    /// it takes, which are what a lens should ask for anyway: an alias
    /// follows the latest model of its line, while the full name the help
    /// gives beside them is an example that pins one release — and not
    /// necessarily the latest. The ones it is known to take follow.
    fn models(&self, help: &str) -> Result<Vec<String>, String> {
        let option = help
            .split("--model")
            .nth(1)
            .map(|rest| {
                // The option's description ends where the next option starts.
                rest.lines()
                    .take_while(|line| {
                        !line.trim_start().starts_with('-') || line.contains("<model>")
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        // Quotes are not paired by counting: an apostrophe in the prose, as in
        // "model's", would shift every pair after it. A name is whatever runs
        // from one quote to the next without a character a name cannot have.
        let quoted = option
            .match_indices('\'')
            .filter_map(|(at, _)| {
                let rest = &option[at + 1..];
                rest.find('\'').map(|end| &rest[..end])
            })
            // An alias is a bare word; a full name has a version in it.
            .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase()))
            .map(str::to_string);
        let mut models: Vec<String> = Vec::new();
        for name in quoted.chain(ALIASES.iter().map(|alias| alias.to_string())) {
            if !models.contains(&name) {
                models.push(name);
            }
        }
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::{invocation, Decoder};
    use super::*;
    use arto_config::LensAgent;
    use indoc::indoc;

    #[test]
    fn claude_runs_bare_and_streams() {
        let run = invocation(&lens(Some(LensAgent::Claude), Some("sonnet")));
        let argv = argv(&run);
        assert_eq!(&argv[1..3], ["-p", "--no-session-persistence"]);
        for flag in [
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--include-partial-messages",
        ] {
            assert!(argv.iter().any(|arg| arg == flag), "{flag}: {argv:?}");
        }
        let tools = argv.iter().position(|arg| arg == "--tools").unwrap();
        assert_eq!(argv[tools + 1], "");
        assert_eq!(&argv[argv.len() - 2..], ["--model", "sonnet"]);
        assert!(!argv.iter().any(|arg| arg == "--allowedTools"));
    }

    #[test]
    fn claude_is_given_and_let_run_the_tools_the_lens_allows() {
        let mut checker = lens(Some(LensAgent::Claude), None);
        checker.allow = vec![LensCapability::WebSearch, LensCapability::Shell];
        let run = invocation(&checker);
        let argv = argv(&run);
        let after = |flag: &str| {
            let at = argv.iter().position(|arg| arg == flag).unwrap();
            argv[at + 1].as_str()
        };
        assert_eq!(after("--tools"), "WebSearch,WebFetch,Bash");
        assert_eq!(after("--allowedTools"), "WebSearch,WebFetch,Bash");
    }

    /// What `claude -p --output-format stream-json --include-partial-messages`
    /// wrote for a short answer, cut to the events that matter and a few
    /// that do not.
    const STREAM: &str = concat!(
        r##"{"type":"system","subtype":"init","session_id":"s"}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"message_start","message":{}}}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"# 見出"}}}"##,
        "\n",
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"し\n\n本文。"}}}"##,
        "\n",
        r##"{"type":"assistant","message":{"content":[{"type":"text","text":"# 見出し\n\n本文。"}]}}"##,
        "\n",
        r##"{"type":"result","subtype":"success","is_error":false,"result":"# 見出し\n\n本文。"}"##,
        "\n",
    );

    #[test]
    fn claude_answers_as_its_deltas_arrive() {
        let mut decoder = Decoder::new(Some(LensAgent::Claude));
        let line_end = |n: usize| STREAM.match_indices('\n').nth(n).unwrap().0 + 1;
        assert_eq!(decoder.feed(&STREAM[..line_end(2)]), "");
        assert_eq!(decoder.feed(&STREAM[..line_end(3)]), "# 見出");
        assert_eq!(decoder.feed(STREAM), "# 見出し\n\n本文。");
        assert_eq!(decoder.finish(STREAM).as_deref(), Ok("# 見出し\n\n本文。"));
    }

    #[test]
    fn claude_streams_only_the_message_after_its_last_tool_call() {
        let stream = concat!(
            r##"{"type":"stream_event","event":{"type":"message_start","message":{"id":"m1"}}}"##,
            "\n",
            r##"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Let me search."}}}"##,
            "\n",
            r##"{"type":"stream_event","event":{"type":"message_start","message":{"id":"m2"}}}"##,
            "\n",
            r##"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Answer"}}}"##,
            "\n",
        );
        let mut decoder = Decoder::new(Some(LensAgent::Claude));
        assert_eq!(decoder.feed(stream), "Answer");
    }

    #[test]
    fn claude_reports_a_failure() {
        let output = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"Credit balance is too low"}"#;
        assert_eq!(
            answer(LensAgent::Claude, output),
            Err("Credit balance is too low".to_string())
        );
    }

    #[test]
    fn claude_offers_the_aliases_its_help_gives_and_the_known_ones() {
        let help = indoc! {"
              --fallback-model <model>              Enable automatic fallback to 'sonnet'
              --model <model>                       Model for the current session. Provide
                                                    an alias for the latest model (e.g.
                                                    'fable', 'opus', or 'sonnet') or a
                                                    model's full name (e.g.
                                                    'claude-fable-5').
              -n, --name <name>                     Set a display name ('mine')
        "};

        assert_eq!(
            Claude.models(help).unwrap(),
            ["fable", "opus", "sonnet", "haiku"]
        );
    }

    #[test]
    fn claude_offers_its_aliases_when_its_help_names_none() {
        assert_eq!(
            Claude.models("Usage: claude").unwrap(),
            ["opus", "sonnet", "haiku"]
        );
    }
}
