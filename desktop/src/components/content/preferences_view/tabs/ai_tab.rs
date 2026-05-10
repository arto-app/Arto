//! Preferences pane for managing AI provider integrations.
//!
//! API keys live in the OS keychain rather than `config.json`. The
//! parent [`PreferencesView`](super::super::main_view::PreferencesView)
//! owns the staging map of unsaved key edits and drives the actual
//! keychain writes/deletes via [`flush_keychain_after_save`] only after
//! `cfg.save()` has succeeded — editing the form has no externally
//! visible side effects until the user clicks Save Changes.

use std::collections::HashMap;

use anyhow::Result;
use dioxus::prelude::*;

use crate::ai;
use crate::components::icon::{Icon, IconName};
use crate::config::{AiAction, AiAuth, AiProtocol, AiProvider, Config};

#[component]
pub fn AiTab(
    config: Signal<Config>,
    has_changes: Signal<bool>,
    pending_secrets: Signal<HashMap<String, String>>,
) -> Element {
    let providers = config.read().ai.providers.clone();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "AI Integrations" }
            p {
                class: "preference-description",
                "Register AI providers (OpenAI, Ollama, Anthropic, …). Each provider \
                 appears in the toolbar AI menu when at least one is configured. \
                 API keys are stored in the OS keychain — never in config.json."
            }

            for (idx, provider) in providers.iter().cloned().enumerate() {
                AiProviderEditor {
                    key: "{provider.id}",
                    index: idx,
                    provider,
                    config,
                    has_changes,
                    pending_secrets,
                }
            }

            div {
                class: "preference-item",
                button {
                    class: "preferences-add-button",
                    onclick: move |_| {
                        config.write().ai.providers.push(default_new_provider());
                        has_changes.set(true);
                    },
                    Icon { name: IconName::Add, size: 16 }
                    span { "Add AI Integration" }
                }
            }
        }
    }
}

#[component]
fn AiProviderEditor(
    index: usize,
    provider: AiProvider,
    config: Signal<Config>,
    has_changes: Signal<bool>,
    pending_secrets: Signal<HashMap<String, String>>,
) -> Element {
    let provider_id = provider.id.clone();
    let pending_key = pending_secrets
        .read()
        .get(&provider_id)
        .cloned()
        .unwrap_or_default();

    let auth_kind = match &provider.auth {
        AiAuth::None => "none",
        AiAuth::Bearer { .. } => "bearer",
        AiAuth::Header { .. } => "header",
    };
    let header_name = match &provider.auth {
        AiAuth::Header { name, .. } => name.clone(),
        _ => String::new(),
    };

    let provider_id_for_remove = provider_id.clone();
    let mut pending_secrets_for_remove = pending_secrets;
    let on_remove = move |_| {
        let mut cfg = config.write();
        if cfg
            .ai
            .providers
            .get(index)
            .is_some_and(|p| p.id == provider_id_for_remove)
        {
            cfg.ai.providers.remove(index);
            // Drop any unsaved secret for this provider so we don't try to
            // re-write it in the post-save flush.
            pending_secrets_for_remove
                .write()
                .remove(&provider_id_for_remove);
            has_changes.set(true);
        }
        // Note: the keychain entry stays alive until Save Changes — the
        // post-save flush sees the provider has disappeared from CONFIG and
        // calls delete_secret then.
    };

    rsx! {
        div {
            class: "preference-item ai-provider-card",

            div {
                class: "ai-provider-card-header",
                input {
                    class: "ai-provider-name",
                    placeholder: "Display name",
                    value: "{provider.name}",
                    oninput: move |e| {
                        if let Some(p) = config.write().ai.providers.get_mut(index) {
                            p.name = e.value();
                        }
                        has_changes.set(true);
                    },
                }
                button {
                    class: "ai-provider-remove",
                    title: "Delete",
                    onclick: on_remove,
                    Icon { name: IconName::Trash, size: 16 }
                }
            }

            // Protocol is currently fixed to OpenAI-compatible. Anthropic is
            // defined in `AiProtocol` but the streaming client doesn't yet
            // implement it, so we don't expose it as a selectable option to
            // avoid letting users save unreachable configurations.
            label { class: "ai-field-label", "Protocol" }
            div {
                class: "ai-protocol-readonly",
                "OpenAI Compatible (OpenAI, Ollama, …)"
            }

            label { class: "ai-field-label", "API URL" }
            input {
                placeholder: "https://api.openai.com/v1/chat/completions",
                value: "{provider.api_url}",
                oninput: move |e| {
                    if let Some(p) = config.write().ai.providers.get_mut(index) {
                        p.api_url = e.value();
                    }
                    has_changes.set(true);
                },
            }

            label { class: "ai-field-label", "Model" }
            input {
                placeholder: "gpt-4o",
                value: "{provider.model}",
                oninput: move |e| {
                    if let Some(p) = config.write().ai.providers.get_mut(index) {
                        p.model = e.value();
                    }
                    has_changes.set(true);
                },
            }

            label { class: "ai-field-label", "Authentication" }
            select {
                value: auth_kind,
                oninput: move |e| {
                    let new_auth = match e.value().as_str() {
                        "bearer" => AiAuth::Bearer { auth_ref: ai::new_auth_ref() },
                        "header" => AiAuth::Header { name: "x-api-key".to_string(), auth_ref: ai::new_auth_ref() },
                        _ => AiAuth::None,
                    };
                    if let Some(p) = config.write().ai.providers.get_mut(index) {
                        p.auth = new_auth;
                    }
                    has_changes.set(true);
                },
                option { value: "none", "None" }
                option { value: "bearer", "Bearer token (Authorization header)" }
                option { value: "header", "Custom header (e.g. x-api-key)" }
            }

            if matches!(provider.auth, AiAuth::Header { .. }) {
                label { class: "ai-field-label", "Header name" }
                input {
                    placeholder: "x-api-key",
                    value: "{header_name}",
                    oninput: move |e| {
                        if let Some(p) = config.write().ai.providers.get_mut(index) {
                            if let AiAuth::Header { ref mut name, .. } = p.auth {
                                *name = e.value();
                            }
                        }
                        has_changes.set(true);
                    },
                }
            }

            if !matches!(provider.auth, AiAuth::None) {
                label { class: "ai-field-label", "API Key (stored in OS keychain)" }
                {
                    let provider_id_for_input = provider_id.clone();
                    rsx! {
                        input {
                            r#type: "password",
                            placeholder: "Leave empty to keep existing key",
                            value: "{pending_key}",
                            oninput: move |e| {
                                let mut map = pending_secrets.write();
                                if e.value().is_empty() {
                                    map.remove(&provider_id_for_input);
                                } else {
                                    map.insert(provider_id_for_input.clone(), e.value());
                                }
                                has_changes.set(true);
                            },
                        }
                    }
                }
            }

            label { class: "ai-field-label", "Action" }
            select {
                value: action_to_str(provider.action),
                oninput: move |e| {
                    let new_action = match e.value().as_str() {
                        "view" => AiAction::View,
                        _ => AiAction::Chat,
                    };
                    if let Some(p) = config.write().ai.providers.get_mut(index) {
                        p.action = new_action;
                    }
                    has_changes.set(true);
                },
                option { value: "chat", "Chat (right sidebar)" }
                option { value: "view", "View (replace document)" }
            }

            label { class: "ai-field-label", "System prompt (optional)" }
            textarea {
                rows: "3",
                value: "{provider.system_prompt.clone().unwrap_or_default()}",
                oninput: move |e| {
                    if let Some(p) = config.write().ai.providers.get_mut(index) {
                        let v = e.value();
                        p.system_prompt = if v.is_empty() { None } else { Some(v) };
                    }
                    has_changes.set(true);
                },
            }

            label { class: "ai-field-label", "Prompt template" }
            p {
                class: "preference-description",
                "Placeholders: {{content}}, {{selection}}, {{path}}, {{title}}"
            }
            textarea {
                rows: "5",
                value: "{provider.prompt_template}",
                oninput: move |e| {
                    if let Some(p) = config.write().ai.providers.get_mut(index) {
                        p.prompt_template = e.value();
                    }
                    has_changes.set(true);
                },
            }
        }
    }
}

/// Reconcile the OS keychain with the result of a successful `cfg.save()`.
///
/// Called by [`PreferencesView`](super::super::main_view::PreferencesView)
/// once the new configuration has been written to disk. Performs two
/// reconciliation passes:
///
/// 1. **Deletes**: for every provider in `previous` whose `auth_ref` is no
///    longer present in `next`, remove the corresponding keychain entry.
///    This covers users deleting a provider in the form.
/// 2. **Writes**: for every entry in `pending_secrets` whose provider still
///    exists in `next`, write the staged secret to the matching `auth_ref`.
///
/// Errors from individual operations are logged but never abort the whole
/// reconciliation, so a transient keychain failure on one entry doesn't
/// strand the rest. Returns `Err` only if the *first* operation fails, to
/// surface obvious misconfigurations to the caller.
pub fn flush_keychain_after_save(
    previous: &Config,
    next: &Config,
    pending_secrets: &HashMap<String, String>,
) -> Result<()> {
    let mut first_error: Option<anyhow::Error> = None;

    let next_auth_refs: std::collections::HashSet<&str> = next
        .ai
        .providers
        .iter()
        .filter_map(|p| secret_auth_ref(&p.auth))
        .collect();

    for previous_provider in &previous.ai.providers {
        let Some(auth_ref) = secret_auth_ref(&previous_provider.auth) else {
            continue;
        };
        if next_auth_refs.contains(auth_ref) {
            continue;
        }
        if let Err(err) = ai::delete_secret(auth_ref) {
            tracing::warn!(?err, auth_ref, "failed to delete keychain entry");
            if first_error.is_none() {
                first_error = Some(err);
            }
        }
    }

    for (provider_id, secret) in pending_secrets {
        let Some(provider) = next.ai.providers.iter().find(|p| &p.id == provider_id) else {
            // Provider was removed in this same save — skip writing the
            // secret since the corresponding keychain entry is also being
            // deleted by the loop above.
            continue;
        };
        let Some(auth_ref) = secret_auth_ref(&provider.auth) else {
            continue;
        };
        if let Err(err) = ai::set_secret(auth_ref, secret) {
            tracing::warn!(?err, %provider_id, "failed to write API key to keychain");
            if first_error.is_none() {
                first_error = Some(err);
            }
        }
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn secret_auth_ref(auth: &AiAuth) -> Option<&str> {
    match auth {
        AiAuth::Bearer { auth_ref } | AiAuth::Header { auth_ref, .. } => Some(auth_ref),
        AiAuth::None => None,
    }
}

fn action_to_str(a: AiAction) -> &'static str {
    match a {
        AiAction::Chat => "chat",
        AiAction::View => "view",
    }
}

fn default_new_provider() -> AiProvider {
    AiProvider {
        id: ai::new_auth_ref(),
        name: "New AI".to_string(),
        icon: String::new(),
        api_url: "https://api.openai.com/v1/chat/completions".to_string(),
        protocol: AiProtocol::OpenAiCompatible,
        model: "gpt-4o-mini".to_string(),
        auth: AiAuth::Bearer {
            auth_ref: ai::new_auth_ref(),
        },
        system_prompt: None,
        prompt_template: "{content}".to_string(),
        action: AiAction::View,
        temperature: None,
    }
}
