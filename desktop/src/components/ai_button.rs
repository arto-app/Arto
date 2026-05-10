//! Toolbar AI button + provider dropdown.
//!
//! Visible only when at least one provider is registered in `Config::ai`.
//! Clicking opens a dropdown of the registered providers; clicking a
//! provider dispatches it against the current tab's content.

use dioxus::prelude::*;

use crate::ai::{render_prompt, stream_chat, AiClientError, AiMessage, PromptInputs, StreamEvent};
use crate::components::icon::{Icon, IconName};
use crate::components::right_sidebar::RightSidebarTab;
use crate::config::{AiAction, AiProvider, CONFIG};
use crate::state::{AiOverlay, AiSession, AppState, TabContent, TabId};

#[component]
pub fn AiButton() -> Element {
    let state = use_context::<AppState>();
    let mut is_open = use_signal(|| false);

    // Snapshot providers reactively so the button hides/shows when the user
    // adds or removes entries via Preferences.
    let providers = use_memo(|| CONFIG.read().ai.providers.clone());
    if providers.read().is_empty() {
        return rsx! {};
    }

    let providers_for_render = providers.read().clone();

    rsx! {
        div {
            class: "ai-button-container",
            style: "position: relative;",

            button {
                class: "nav-button ai-button",
                class: if *is_open.read() { "active" },
                title: "AI integrations",
                onclick: move |_| is_open.toggle(),
                Icon { name: IconName::Sparkles, size: 20 }
            }

            if *is_open.read() {
                div {
                    class: "ai-button-backdrop",
                    onclick: move |_| is_open.set(false),
                }
                div {
                    class: "ai-button-dropdown",
                    for provider in providers_for_render.iter().cloned() {
                        AiDropdownItem {
                            provider: provider.clone(),
                            on_invoke: move |p: AiProvider| {
                                is_open.set(false);
                                dispatch_provider(state, p);
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AiDropdownItem(provider: AiProvider, on_invoke: EventHandler<AiProvider>) -> Element {
    let label = format!(
        "{} ({})",
        provider.name,
        match provider.action {
            AiAction::Chat => "Chat",
            AiAction::View => "View",
        }
    );
    rsx! {
        button {
            class: "ai-dropdown-item",
            onclick: move |_| on_invoke.call(provider.clone()),
            Icon { name: IconName::Wand, size: 16 }
            span { class: "ai-dropdown-item-label", "{label}" }
        }
    }
}

/// Dispatch the chosen provider against the current tab.
///
/// View actions populate `state.ai_overlays[active_tab]`. Chat actions
/// instead start a session in `state.ai_chat_sessions` and switch the
/// right sidebar to the AI tab.
fn dispatch_provider(state: AppState, provider: AiProvider) {
    let active = *state.active_tab.read();
    let tab_snapshot = match state.tabs.read().get(active).cloned() {
        Some(tab) => tab,
        None => return,
    };

    let content = match tab_content_to_markdown(&tab_snapshot.content) {
        Some(c) => c,
        None => {
            tracing::debug!("AI invoked on tab without markdown content; ignoring");
            return;
        }
    };

    let tab_id = tab_snapshot.tab_id;
    match provider.action {
        AiAction::View => start_view_action(state, provider, content, tab_snapshot, tab_id),
        AiAction::Chat => start_chat_action(state, provider, content, tab_snapshot, tab_id),
    }
}

/// Extract Markdown source from a tab's content. Returns `None` for tabs
/// that don't contain Markdown (e.g. Preferences).
fn tab_content_to_markdown(content: &TabContent) -> Option<String> {
    match content {
        TabContent::File(path) => std::fs::read_to_string(path).ok(),
        TabContent::Inline(md) => Some(md.clone()),
        TabContent::FileError(_, _) | TabContent::None | TabContent::Preferences => None,
    }
}

fn start_view_action(
    mut state: AppState,
    provider: AiProvider,
    content: String,
    tab: crate::state::Tab,
    tab_id: TabId,
) {
    // Build prompt from the tab's content.
    let path = tab.file().map(std::path::PathBuf::from);
    let inputs = PromptInputs::from_parts(&content, "", path.as_deref());
    let user_prompt = render_prompt(&provider.prompt_template, &inputs);

    let mut messages = Vec::new();
    if let Some(sys) = provider.system_prompt.as_deref() {
        if !sys.is_empty() {
            messages.push(AiMessage::system(sys));
        }
    }
    messages.push(AiMessage::user(user_prompt));

    let (handle, mut rx) = match stream_chat(&provider, messages) {
        Ok(pair) => pair,
        Err(err) => {
            tracing::error!(?err, "failed to start AI request");
            install_error_overlay(&mut state, tab_id, &provider, format_error(&err));
            return;
        }
    };

    // Seed the overlay so the UI shows a streaming placeholder immediately.
    let overlay = AiOverlay::new(provider.id.clone(), provider.name.clone());
    state.ai_overlays.write().insert(tab_id, overlay);

    // Drop handle once we're done; cancellation hooks land later.
    let _ = handle;

    spawn(async move {
        while let Some(event) = rx.recv().await {
            let mut map = state.ai_overlays.write();
            let Some(entry) = map.get_mut(&tab_id) else {
                // User cleared the overlay (or closed the tab). Stop draining.
                break;
            };
            match event {
                StreamEvent::Delta(chunk) => entry.append_delta(&chunk),
                StreamEvent::Done => {
                    entry.finish_ok();
                    break;
                }
                StreamEvent::Error(msg) => {
                    entry.finish_err(msg);
                    break;
                }
            }
        }
    });
}

fn start_chat_action(
    mut state: AppState,
    provider: AiProvider,
    content: String,
    tab: crate::state::Tab,
    tab_id: TabId,
) {
    // Build the initial user prompt (document context) and any system message.
    let path = tab.file().map(std::path::PathBuf::from);
    let inputs = PromptInputs::from_parts(&content, "", path.as_deref());
    let initial_user = render_prompt(&provider.prompt_template, &inputs);

    let mut messages = Vec::new();
    if let Some(sys) = provider.system_prompt.as_deref() {
        if !sys.is_empty() {
            messages.push(AiMessage::system(sys));
        }
    }
    messages.push(AiMessage::user(initial_user.clone()));

    // Seed the visible session: show the user turn we just sent, then a
    // placeholder assistant turn that streaming deltas append to.
    let mut session = AiSession::new(provider.id.clone(), provider.name.clone());
    session.push_user(initial_user);
    session.begin_assistant_turn();
    state.ai_chat_sessions.write().insert(tab_id, session);

    // Reveal the chat panel by switching the right sidebar to the AI tab.
    state.right_sidebar_tab.set(RightSidebarTab::Ai);
    state.right_sidebar_pinned.set(true);

    let (handle, mut rx) = match stream_chat(&provider, messages) {
        Ok(pair) => pair,
        Err(err) => {
            if let Some(s) = state.ai_chat_sessions.write().get_mut(&tab_id) {
                s.finish_err(format_error(&err));
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

fn install_error_overlay(
    state: &mut AppState,
    tab_id: TabId,
    provider: &AiProvider,
    message: String,
) {
    let mut overlay = AiOverlay::new(provider.id.clone(), provider.name.clone());
    overlay.finish_err(message);
    state.ai_overlays.write().insert(tab_id, overlay);
}

fn format_error(err: &AiClientError) -> String {
    match err {
        AiClientError::MissingSecret { auth_ref } => format!(
            "API key for this provider is missing in the keychain (auth_ref: {auth_ref}). \
             Open Preferences → AI to set it."
        ),
        AiClientError::UnsupportedProtocol => {
            "This provider's protocol is not yet supported.".to_string()
        }
        AiClientError::Other(e) => format!("{e:#}"),
    }
}
