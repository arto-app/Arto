use serde::{Deserialize, Serialize};

/// Top-level AI configuration: a list of user-registered AI providers.
///
/// Stored under `ai` in `config.json`. When this list is empty, the AI toolbar
/// button is hidden. Authentication secrets are NOT stored here — only an
/// opaque `auth_ref` identifier that maps to a keychain entry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    #[serde(default)]
    pub providers: Vec<AiProvider>,
}

/// A single registered AI provider configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    /// Stable identifier (UUID v4). Used as React-key-style identity for the
    /// dropdown and as a join key with per-tab session state.
    pub id: String,
    /// Display name shown in the toolbar dropdown and Preferences list.
    pub name: String,
    /// Tabler icon name (matches `IconName` registered via `add-icon` skill).
    /// Stored as a plain string — the UI maps it to `IconName` at render time
    /// and falls back to a generic AI icon if unknown.
    #[serde(default)]
    pub icon: String,
    /// API endpoint, e.g. `https://api.openai.com/v1/chat/completions` or
    /// `http://localhost:11434/v1/chat/completions` for Ollama.
    pub api_url: String,
    /// API protocol shape — selects the request/response (de)serializer.
    #[serde(default)]
    pub protocol: AiProtocol,
    /// Model identifier passed in the request body.
    pub model: String,
    /// Auth scheme; the actual secret lives in the OS keychain under `auth_ref`.
    #[serde(default)]
    pub auth: AiAuth,
    /// Optional system prompt prepended as a `system` message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Prompt template for the user message. Supports placeholders:
    /// `{content}`, `{selection}`, `{path}`, `{title}`.
    pub prompt_template: String,
    /// What to do with the AI response.
    #[serde(default)]
    pub action: AiAction,
    /// Sampling temperature passed through to the provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// API protocol shape. OpenAI-compatible covers OpenAI itself plus Ollama,
/// vLLM, LM Studio, and most OSS inference servers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProtocol {
    #[default]
    OpenAiCompatible,
    Anthropic,
}

/// Authentication scheme. Secrets never live in `config.json` — only the
/// `auth_ref` identifier that points to a keychain entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum AiAuth {
    /// No authentication (e.g. local Ollama with no auth).
    #[default]
    None,
    /// `Authorization: Bearer <secret>` header.
    Bearer { auth_ref: String },
    /// Custom header (e.g. `x-api-key` for Anthropic).
    Header { name: String, auth_ref: String },
}

/// What to do with the AI response.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAction {
    /// Open a chat panel in the right sidebar with streaming response.
    #[default]
    Chat,
    /// Replace the current tab's rendered view with the AI output.
    View,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_provider() -> AiProvider {
        AiProvider {
            id: "11111111-2222-3333-4444-555555555555".to_string(),
            name: "Translate".to_string(),
            icon: "language".to_string(),
            api_url: "https://api.openai.com/v1/chat/completions".to_string(),
            protocol: AiProtocol::OpenAiCompatible,
            model: "gpt-4o".to_string(),
            auth: AiAuth::Bearer {
                auth_ref: "abc-123".to_string(),
            },
            system_prompt: Some("You are a translator.".to_string()),
            prompt_template: "Translate to Japanese:\n\n{content}".to_string(),
            action: AiAction::View,
            temperature: Some(0.3),
        }
    }

    #[test]
    fn ai_config_default_is_empty() {
        let cfg = AiConfig::default();
        assert!(cfg.providers.is_empty());
    }

    #[test]
    fn ai_protocol_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&AiProtocol::OpenAiCompatible).unwrap(),
            r#""open_ai_compatible""#
        );
        assert_eq!(
            serde_json::to_string(&AiProtocol::Anthropic).unwrap(),
            r#""anthropic""#
        );
    }

    #[test]
    fn ai_action_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&AiAction::Chat).unwrap(), r#""chat""#);
        assert_eq!(serde_json::to_string(&AiAction::View).unwrap(), r#""view""#);
    }

    #[test]
    fn ai_auth_none_serializes_with_type_tag_only() {
        let json = serde_json::to_string(&AiAuth::None).unwrap();
        assert_eq!(json, r#"{"type":"none"}"#);

        let parsed: AiAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AiAuth::None);
    }

    #[test]
    fn ai_auth_bearer_roundtrips() {
        let auth = AiAuth::Bearer {
            auth_ref: "ref-1".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        // tagged enum + camelCase field renaming
        assert!(json.contains(r#""type":"bearer""#));
        assert!(json.contains(r#""authRef":"ref-1""#));

        let parsed: AiAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, auth);
    }

    #[test]
    fn ai_auth_header_roundtrips() {
        let auth = AiAuth::Header {
            name: "x-api-key".to_string(),
            auth_ref: "ref-2".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        let parsed: AiAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, auth);
    }

    #[test]
    fn ai_provider_roundtrips() {
        let p = sample_provider();
        let json = serde_json::to_string_pretty(&p).unwrap();
        let parsed: AiProvider = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, p);

        // Verify camelCase field names appear in JSON.
        assert!(json.contains(r#""apiUrl""#));
        assert!(json.contains(r#""systemPrompt""#));
        assert!(json.contains(r#""promptTemplate""#));
    }

    #[test]
    fn ai_provider_omits_optional_fields_when_none() {
        let p = AiProvider {
            id: "id".to_string(),
            name: "name".to_string(),
            icon: String::new(),
            api_url: "url".to_string(),
            protocol: AiProtocol::default(),
            model: "m".to_string(),
            auth: AiAuth::None,
            system_prompt: None,
            prompt_template: String::new(),
            action: AiAction::default(),
            temperature: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("systemPrompt"));
        assert!(!json.contains("temperature"));
    }

    #[test]
    fn ai_config_roundtrips_with_multiple_providers() {
        let cfg = AiConfig {
            providers: vec![
                sample_provider(),
                AiProvider {
                    id: "other".to_string(),
                    name: "Summarize".to_string(),
                    icon: String::new(),
                    api_url: "http://localhost:11434/v1/chat/completions".to_string(),
                    protocol: AiProtocol::OpenAiCompatible,
                    model: "llama3".to_string(),
                    auth: AiAuth::None,
                    system_prompt: None,
                    prompt_template: "Summarize: {content}".to_string(),
                    action: AiAction::Chat,
                    temperature: None,
                },
            ],
        };

        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: AiConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cfg);
        assert_eq!(parsed.providers.len(), 2);
    }

    #[test]
    fn ai_config_missing_section_uses_default() {
        // When `ai` is missing from the top-level JSON, AiConfig::default()
        // should be used (verified at the Config level via #[serde(default)]).
        let json = r#"{}"#;
        let parsed: AiConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, AiConfig::default());
    }
}
