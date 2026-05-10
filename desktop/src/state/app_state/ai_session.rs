//! Per-tab AI chat session.
//!
//! Each tab can host one in-flight chat session keyed by tab index in
//! `AppState::ai_chat_sessions`. Messages live in memory only — chats are
//! ephemeral and not persisted across window reopens.

/// Role of a single message in an [`AiSession`].
///
/// System prompts are sent to the provider via the wire-format messages but
/// not surfaced in the visible transcript, so this enum only models the two
/// roles a user actually sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

/// One message in the chat transcript.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatTurn {
    pub role: ChatRole,
    pub content: String,
}

/// State of an AI chat conversation attached to a single tab.
#[derive(Debug, Clone, PartialEq)]
pub struct AiSession {
    pub provider_id: String,
    pub provider_name: String,
    /// Visible transcript. The leading message is typically the document
    /// the user opened the chat against, surfaced as a `User` turn.
    pub turns: Vec<ChatTurn>,
    /// True while a response is streaming. Used to disable the input box
    /// and show a spinner.
    pub streaming: bool,
    /// Error from the last streaming attempt, surfaced under the transcript.
    pub error: Option<String>,
}

impl AiSession {
    pub fn new(provider_id: impl Into<String>, provider_name: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            provider_name: provider_name.into(),
            turns: Vec::new(),
            streaming: false,
            error: None,
        }
    }

    pub fn push_user(&mut self, content: impl Into<String>) {
        self.turns.push(ChatTurn {
            role: ChatRole::User,
            content: content.into(),
        });
    }

    /// Begin a new assistant turn that will be filled in by streaming deltas.
    pub fn begin_assistant_turn(&mut self) {
        self.turns.push(ChatTurn {
            role: ChatRole::Assistant,
            content: String::new(),
        });
        self.streaming = true;
        self.error = None;
    }

    /// Append a delta to the most recent assistant turn (started via
    /// [`begin_assistant_turn`]). No-op if the last turn is not assistant.
    pub fn append_delta(&mut self, delta: &str) {
        if let Some(last) = self.turns.last_mut() {
            if last.role == ChatRole::Assistant {
                last.content.push_str(delta);
            }
        }
    }

    pub fn finish_ok(&mut self) {
        self.streaming = false;
    }

    pub fn finish_err(&mut self, message: impl Into<String>) {
        self.streaming = false;
        self.error = Some(message.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_has_no_turns() {
        let s = AiSession::new("p", "Translator");
        assert!(s.turns.is_empty());
        assert!(!s.streaming);
        assert!(s.error.is_none());
    }

    #[test]
    fn push_user_appends_user_turn() {
        let mut s = AiSession::new("p", "n");
        s.push_user("hello");
        assert_eq!(s.turns.len(), 1);
        assert_eq!(s.turns[0].role, ChatRole::User);
        assert_eq!(s.turns[0].content, "hello");
    }

    #[test]
    fn assistant_streaming_appends_to_last_turn() {
        let mut s = AiSession::new("p", "n");
        s.push_user("hi");
        s.begin_assistant_turn();
        s.append_delta("Hel");
        s.append_delta("lo!");
        assert_eq!(s.turns.len(), 2);
        assert_eq!(s.turns[1].role, ChatRole::Assistant);
        assert_eq!(s.turns[1].content, "Hello!");
        assert!(s.streaming);

        s.finish_ok();
        assert!(!s.streaming);
        assert!(s.error.is_none());
    }

    #[test]
    fn append_delta_when_last_turn_is_user_is_noop() {
        let mut s = AiSession::new("p", "n");
        s.push_user("hi");
        s.append_delta("ignored");
        assert_eq!(s.turns.len(), 1);
        assert_eq!(s.turns[0].content, "hi");
    }

    #[test]
    fn finish_err_records_message() {
        let mut s = AiSession::new("p", "n");
        s.begin_assistant_turn();
        s.finish_err("network");
        assert!(!s.streaming);
        assert_eq!(s.error.as_deref(), Some("network"));
    }
}
