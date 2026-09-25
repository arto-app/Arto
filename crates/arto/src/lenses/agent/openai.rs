//! A server with an OpenAI-compatible chat completions API — OpenAI itself,
//! LM Studio, llama.cpp and the like — which streams server-sent events.

use super::{text, Answer, Reader, ServerAgent};
use serde_json::Value;

pub(super) struct OpenAi;

impl ServerAgent for OpenAi {
    fn chat_path(&self) -> &'static str {
        "/chat/completions"
    }

    fn models_path(&self) -> &'static str {
        "/models"
    }
}

impl Reader for OpenAi {
    /// A server-sent event carries its JSON after `data:`.
    fn event<'a>(&self, line: &'a str) -> &'a str {
        line.strip_prefix("data:").map_or(line, str::trim)
    }

    fn read(&self, event: &Value, answer: &mut Answer) {
        let choice = &event["choices"][0];
        if let Some(error) = text(&event["error"]["message"]).or_else(|| text(&event["error"])) {
            answer.fail(error);
        } else if let Some(content) = choice["delta"]["content"].as_str() {
            answer.push(content);
        } else if let Some(content) = text(&choice["message"]["content"]) {
            // A server that does not stream answers whole.
            answer.report(content);
        }
    }

    /// The models the server lists at `/models`, in name order.
    fn models(&self, list: &str) -> Result<Vec<String>, String> {
        let list: Value = serde_json::from_str(list).map_err(|error| error.to_string())?;
        let mut models: Vec<String> = list["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| model["id"].as_str().map(str::to_string))
            .collect();
        models.sort();
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::{invocation, Decoder};
    use super::*;
    use arto_config::LensAgent;

    #[test]
    fn an_openai_server_is_not_sent_what_only_ollama_takes() {
        let run = invocation(&lens(Some(LensAgent::Openai), Some("m")));
        let body: Value = serde_json::from_str(&server(&run).body("x")).unwrap();
        assert!(body.get("options").is_none(), "{body}");
    }

    /// What an OpenAI-compatible server streams for a short answer.
    const STREAM: &str = concat!(
        r#"data: {"id":"c","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":""}}]}"#,
        "\n\n",
        r#"data: {"id":"c","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"こんにち"}}]}"#,
        "\n\n",
        r#"data: {"id":"c","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"は"},"finish_reason":"stop"}]}"#,
        "\n\n",
        "data: [DONE]\n\n",
    );

    #[test]
    fn an_openai_server_answers_as_its_events_arrive() {
        let mut decoder = Decoder::new(Some(LensAgent::Openai));
        let two_events = STREAM.match_indices("\n\n").nth(1).unwrap().0 + 2;
        assert_eq!(decoder.feed(&STREAM[..two_events]), "こんにち");
        assert_eq!(decoder.finish(STREAM).as_deref(), Ok("こんにちは"));
    }

    #[test]
    fn an_openai_server_that_does_not_stream_answers_whole() {
        let output = r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"全文"}}]}"#;
        assert_eq!(answer(LensAgent::Openai, output).as_deref(), Ok("全文"));
    }

    #[test]
    fn an_openai_server_reports_a_failure() {
        let output =
            r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#;
        assert_eq!(
            answer(LensAgent::Openai, output),
            Err("Incorrect API key provided".to_string())
        );
    }

    #[test]
    fn an_openai_server_offers_its_list_in_name_order() {
        let list = r#"{"object": "list", "data": [{"id": "gpt-b"}, {"id": "gpt-a"}]}"#;

        assert_eq!(OpenAi.models(list).unwrap(), ["gpt-a", "gpt-b"]);
    }
}
