//! OpenAI Chat Completions request/response shapes.
//!
//! Compatible with OpenAI itself, Ollama, vLLM, LM Studio, and most OSS
//! inference servers that implement the OpenAI API.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<ChatMessage<'a>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

/// Streaming chunk: `{"choices":[{"delta":{"content":"..."}}]}`.
#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    #[serde(default)]
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    #[serde(default)]
    pub delta: Delta,
}

#[derive(Debug, Default, Deserialize)]
pub struct Delta {
    #[serde(default)]
    pub content: Option<String>,
}

/// Extract the text delta from one parsed SSE `data:` payload, if any.
pub fn extract_delta(chunk: &StreamChunk) -> Option<&str> {
    chunk
        .choices
        .first()
        .and_then(|c| c.delta.content.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_streaming_chunk() {
        let json = r#"{"id":"x","choices":[{"delta":{"content":"Hello"}}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(extract_delta(&chunk), Some("Hello"));
    }

    #[test]
    fn parses_chunk_without_content() {
        // First chunk often only has role assignment.
        let json = r#"{"id":"x","choices":[{"delta":{"role":"assistant"}}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(extract_delta(&chunk), None);
    }

    #[test]
    fn parses_chunk_with_empty_choices() {
        // Final chunk for some providers has empty choices.
        let json = r#"{"id":"x","choices":[]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(extract_delta(&chunk), None);
    }

    #[test]
    fn request_serializes_with_messages() {
        let req = ChatRequest {
            model: "gpt-4o",
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: "be helpful",
                },
                ChatMessage {
                    role: "user",
                    content: "hi",
                },
            ],
            stream: true,
            temperature: Some(0.5),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""model":"gpt-4o""#));
        assert!(json.contains(r#""stream":true"#));
        assert!(json.contains(r#""temperature":0.5"#));
        assert!(json.contains(r#""role":"system""#));
        assert!(json.contains(r#""content":"hi""#));
    }

    #[test]
    fn request_omits_none_temperature() {
        let req = ChatRequest {
            model: "m",
            messages: vec![],
            stream: false,
            temperature: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("temperature"));
    }
}
