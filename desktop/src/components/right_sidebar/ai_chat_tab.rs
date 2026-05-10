//! Right-sidebar Chat tab.
//!
//! Renders the [`AiSession`] for the active tab, and accepts a follow-up
//! message from the user. The session is opened by `AiButton`'s dispatcher
//! when a Chat-action provider is selected; this component does not start
//! sessions on its own.

use dioxus::prelude::*;
use std::path::Path;

use crate::ai::{stream_chat, AiMessage, StreamEvent};
use crate::config::CONFIG;
use crate::markdown::render_to_html;
use crate::state::{AiSession, AppState, ChatTurn, ChatTurnRole, TabId};

#[component]
pub fn AiChatTab() -> Element {
    let mut state = use_context::<AppState>();
    let active_tab_id = use_memo(move || state.current_tab().map(|tab| tab.tab_id));
    let session = use_memo(move || {
        active_tab_id()
            .and_then(|id| state.ai_chat_sessions.read().get(&id).cloned())
    });
    let mut input = use_signal(String::new);

    let Some(session) = session() else {
        return rsx! {
            div {
                class: "ai-chat-empty",
                p { "No AI chat is active for this tab." }
                p {
                    class: "ai-chat-empty-hint",
                    "Select a provider with the ✨ AI menu in the toolbar."
                }
            }
        };
    };

    let provider_name = session.provider_name.clone();
    let provider_id = session.provider_id.clone();
    let streaming = session.streaming;
    let error = session.error.clone();
    let turns = session.turns.clone();
    let tab_id = active_tab_id().unwrap_or_default();

    let close = move |_| {
        state.ai_chat_sessions.write().remove(&tab_id);
    };

    let send_message = move |_| {
        let text = input.read().trim().to_string();
        if text.is_empty() || streaming {
            return;
        }
        input.set(String::new());
        send_followup(state, tab_id, provider_id.clone(), text);
    };

    rsx! {
        div {
            class: "ai-chat",

            div {
                class: "ai-chat-header",
                span { class: "ai-chat-title", "{provider_name}" }
                button {
                    class: "ai-chat-close",
                    title: "Close session",
                    onclick: close,
                    "×"
                }
            }

            div {
                class: "ai-chat-transcript",
                for (i, turn) in turns.iter().enumerate() {
                    AiChatBubble {
                        key: "{i}",
                        turn: turn.clone(),
                    }
                }
                if streaming {
                    div { class: "ai-chat-typing", "Streaming…" }
                }
                if let Some(err) = error {
                    div { class: "ai-chat-error", "{err}" }
                }
            }

            div {
                class: "ai-chat-input-row",
                textarea {
                    class: "ai-chat-input",
                    rows: "2",
                    placeholder: "Ask a follow-up…",
                    disabled: streaming,
                    value: "{input}",
                    oninput: move |e| input.set(e.value()),
                }
                button {
                    class: "ai-chat-send",
                    disabled: streaming || input.read().trim().is_empty(),
                    onclick: send_message,
                    "Send"
                }
            }
        }
    }
}

#[component]
fn AiChatBubble(turn: ChatTurn) -> Element {
    let html = use_signal(String::new);
    let content = turn.content.clone();
    use_chat_markdown_render(content.clone(), html);

    let role_class = match turn.role {
        ChatTurnRole::Assistant => "ai-chat-bubble assistant",
        ChatTurnRole::User => "ai-chat-bubble user",
    };

    rsx! {
        div {
            class: "{role_class}",
            div {
                class: "markdown-body",
                dangerous_inner_html: "{html}"
            }
        }
    }
}

fn use_chat_markdown_render(markdown: String, html: Signal<String>) {
    use_effect(move || {
        let mut html = html;
        let markdown = markdown.clone();
        spawn(async move {
            let rendered = render_to_html(&markdown, Path::new(".")).unwrap_or_else(|e| {
                tracing::debug!(?e, "failed to render chat bubble markdown");
                html_escape::encode_safe(&markdown).into_owned()
            });
            html.set(rendered);
        });
    });
}

/// Re-issue the chat against the provider with a new user turn appended.
fn send_followup(mut state: AppState, tab_id: TabId, provider_id: String, user_text: String) {
    let provider = {
        let cfg = CONFIG.read();
        match cfg.ai.providers.iter().find(|p| p.id == provider_id) {
            Some(p) => p.clone(),
            None => {
                tracing::warn!(%provider_id, "provider missing; chat session orphaned");
                return;
            }
        }
    };

    // Build the message list from the current transcript, then append the
    // new user turn and a placeholder assistant turn for streaming into.
    // The provider's system prompt is re-prepended every time because Chat
    // Completions APIs are stateless — omitting it on follow-ups silently
    // drops the operator's instructions after the first turn.
    let messages = {
        let mut sessions = state.ai_chat_sessions.write();
        let Some(session) = sessions.get_mut(&tab_id) else {
            return;
        };
        session.push_user(user_text.clone());
        let mut messages = Vec::new();
        if let Some(sys) = provider.system_prompt.as_deref() {
            if !sys.is_empty() {
                messages.push(AiMessage::system(sys));
            }
        }
        messages.extend(session_to_messages(session));
        session.begin_assistant_turn();
        messages
    };

    let (handle, mut rx) = match stream_chat(&provider, messages) {
        Ok(pair) => pair,
        Err(err) => {
            if let Some(session) = state.ai_chat_sessions.write().get_mut(&tab_id) {
                session.finish_err(format!("{err:#}"));
            }
            return;
        }
    };
    let _ = handle;

    spawn(async move {
        while let Some(event) = rx.recv().await {
            let mut sessions = state.ai_chat_sessions.write();
            let Some(session) = sessions.get_mut(&tab_id) else {
                break;
            };
            match event {
                StreamEvent::Delta(d) => session.append_delta(&d),
                StreamEvent::Done => {
                    session.finish_ok();
                    break;
                }
                StreamEvent::Error(msg) => {
                    session.finish_err(msg);
                    break;
                }
            }
        }
    });
}

/// Convert the session transcript into the wire-format messages that the
/// streaming client expects. Drops the in-progress assistant turn (if any)
/// so we don't send empty content.
fn session_to_messages(session: &AiSession) -> Vec<AiMessage> {
    session
        .turns
        .iter()
        .filter_map(|t| {
            if matches!(t.role, ChatTurnRole::Assistant) && t.content.is_empty() {
                None
            } else {
                Some(match t.role {
                    ChatTurnRole::User => AiMessage::user(t.content.clone()),
                    ChatTurnRole::Assistant => AiMessage::assistant(t.content.clone()),
                })
            }
        })
        .collect()
}
