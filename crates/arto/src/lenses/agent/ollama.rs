//! An Ollama server, asked through its own API rather than its
//! OpenAI-compatible one: only its own takes the context length with each
//! request, and left to its default a long-context model loads a context of
//! tens of gigabytes.

use super::{chat_body, text, Answer, Reader, Server, ServerAgent};
use serde_json::{json, Value};

/// The bounds of the context a model is loaded with when the lens leaves
/// it to Arto.
const MIN_CONTEXT: usize = 4096;
const MAX_CONTEXT: usize = 131_072;

pub(super) struct Ollama;

impl ServerAgent for Ollama {
    fn chat_path(&self) -> &'static str {
        "/api/chat"
    }

    fn models_path(&self) -> &'static str {
        "/api/tags"
    }

    fn body(&self, server: &Server, message: &str) -> Value {
        let mut body = chat_body(server, message);
        let context = server
            .context_length
            .map_or_else(|| context_for(message), |length| length as usize);
        body["options"] = json!({ "num_ctx": context });
        body
    }
}

impl Reader for Ollama {
    fn read(&self, event: &Value, answer: &mut Answer) {
        if let Some(error) = text(&event["error"]) {
            answer.fail(error);
        } else if let Some(content) = event["message"]["content"].as_str() {
            answer.push(content);
        }
    }

    /// The models the server has pulled, from `/api/tags`.
    fn models(&self, tags: &str) -> Result<Vec<String>, String> {
        let tags: Value = serde_json::from_str(tags).map_err(|error| error.to_string())?;
        Ok(tags["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| model["name"].as_str().map(str::to_string))
            .collect())
    }
}

/// A context that holds `message` and an answer as long as it, rounded up
/// so that requests of a similar size share a loaded model. A character is
/// counted as a token, which overestimates English and fits Japanese.
fn context_for(message: &str) -> usize {
    let needed = message.chars().count() * 2 + 1024;
    needed.next_power_of_two().clamp(MIN_CONTEXT, MAX_CONTEXT)
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::{invocation, Decoder};
    use super::*;
    use arto_config::LensAgent;

    #[test]
    fn ollama_is_asked_with_a_context_sized_to_the_request() {
        let run = invocation(&lens(Some(LensAgent::Ollama), Some("qwen3:4b-instruct")));
        let server = server(&run);

        let body: Value = serde_json::from_str(&server.body("short")).unwrap();
        assert_eq!(body["model"], "qwen3:4b-instruct");
        assert_eq!(
            body["messages"],
            json!([{"role": "user", "content": "short"}])
        );
        assert_eq!(body["options"]["num_ctx"], MIN_CONTEXT);

        let long: Value = serde_json::from_str(&server.body(&"語".repeat(10_000))).unwrap();
        assert_eq!(long["options"]["num_ctx"], 32_768);
    }

    #[test]
    fn a_context_length_the_lens_names_wins() {
        let mut ollama = lens(Some(LensAgent::Ollama), Some("m"));
        ollama.context_length = Some(8192);
        let body: Value = serde_json::from_str(&server(&invocation(&ollama)).body("x")).unwrap();
        assert_eq!(body["options"]["num_ctx"], 8192);
    }

    #[test]
    fn a_context_is_bounded() {
        assert_eq!(context_for(""), MIN_CONTEXT);
        assert_eq!(context_for(&"x".repeat(1_000_000)), MAX_CONTEXT);
    }

    /// What `/api/chat` streams for a short answer.
    const CHAT: &str = concat!(
        r##"{"model":"m","message":{"role":"assistant","content":"# 見"},"done":false}"##,
        "\n",
        r##"{"model":"m","message":{"role":"assistant","content":"出し\n\n本文。"},"done":false}"##,
        "\n",
        r##"{"model":"m","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop"}"##,
        "\n",
    );

    #[test]
    fn ollama_answers_as_its_messages_arrive() {
        let mut decoder = Decoder::new(Some(LensAgent::Ollama));
        let first_line = CHAT.find('\n').unwrap() + 1;
        assert_eq!(decoder.feed(&CHAT[..first_line]), "# 見");
        assert_eq!(decoder.finish(CHAT).as_deref(), Ok("# 見出し\n\n本文。"));
    }

    #[test]
    fn ollama_reports_a_failure() {
        let output = r#"{"error":"model \"qwen9\" not found, try pulling it first"}"#;
        assert_eq!(
            answer(LensAgent::Ollama, output),
            Err(r#"model "qwen9" not found, try pulling it first"#.to_string())
        );
    }

    #[test]
    fn ollama_offers_what_it_has_pulled() {
        let tags = r#"{"models": [{"name": "qwen3:4b-instruct", "size": 1}]}"#;

        assert_eq!(Ollama.models(tags).unwrap(), ["qwen3:4b-instruct"]);
    }
}
