use dioxus::prelude::*;
use rust_i18n::t;

use super::argv;
use crate::lenses::{available_models, forget_key, is_stored, store_key, ModelSource};

#[component]
pub(super) fn TextField(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: String,
    #[props(default)] monospace: bool,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", "{label}" }
            input {
                class: "lens-input",
                class: if monospace { "monospace" },
                r#type: "text",
                spellcheck: false,
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}

#[component]
pub(super) fn TextArea(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: String,
    rows: u32,
    #[props(default)] monospace: bool,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", "{label}" }
            textarea {
                class: "lens-input",
                class: if monospace { "monospace" },
                rows: "{rows}",
                spellcheck: false,
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}

/// The most suggestions shown as buttons; a server with more — a hosted
/// catalog — is left to the field's own completion.
const MAX_MODEL_CHIPS: usize = 12;

/// The model, with the ones the agent offers to pick from: asked of the
/// agent itself when the lens is opened, and again only when where it is
/// asked changes — not as the prompt is typed.
#[component]
pub(super) fn ModelField(
    index: usize,
    source: ReadSignal<Option<ModelSource>>,
    /// Changes when the stored API key does, to ask again.
    key_revision: ReadSignal<u64>,
    #[props(into)] hint: String,
    value: String,
    on_input: EventHandler<String>,
) -> Element {
    let offered = use_resource(move || async move {
        let _ = key_revision();
        match source() {
            Some(source) => available_models(&source).await,
            None => Ok(Vec::new()),
        }
    });
    let list_id = format!("lens-{index}-models");
    let (models, status) = match &*offered.read() {
        None => (
            Vec::new(),
            Some(t!("preferences.lenses.model.asking").into_owned()),
        ),
        Some(Ok(models)) => (models.clone(), None),
        Some(Err(reason)) => (
            Vec::new(),
            Some(t!("preferences.lenses.model.unlisted", reason = reason).into_owned()),
        ),
    };

    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", {t!("preferences.lenses.model.label").to_string()} }
            input {
                class: "lens-input",
                r#type: "text",
                spellcheck: false,
                list: "{list_id}",
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            datalist {
                id: "{list_id}",
                for model in models.iter() {
                    option { key: "{model}", value: "{model}" }
                }
            }
            if !models.is_empty() && models.len() <= MAX_MODEL_CHIPS {
                div {
                    class: "lens-model-chips",
                    for model in models.iter().cloned() {
                        button {
                            key: "{model}",
                            class: "lens-model-chip",
                            class: if model == value { "selected" },
                            r#type: "button",
                            onclick: {
                                let model = model.clone();
                                move |_| on_input.call(model.clone())
                            },
                            "{model}"
                        }
                    }
                }
            }
            span { class: "lens-field-hint", "{hint}" }
            if let Some(status) = status {
                span { class: "lens-field-hint lens-field-status", "{status}" }
            }
        }
    }
}

/// The server's API key, kept in the system's credential store under its
/// endpoint rather than in `config.json`: typed once, never shown again,
/// and shared by every lens that asks the same server.
///
/// Saved on a button rather than as it is typed, unlike every other field
/// here: each keystroke would otherwise be a write to the store, and half a
/// key is not a key.
#[component]
pub(super) fn ApiKeyField(
    account: ReadSignal<Option<String>>,
    /// Whether an API key command takes the stored key's place.
    overridden: bool,
    on_change: EventHandler<()>,
) -> Element {
    let mut revision = use_signal(|| 0_u64);
    let stored = use_resource(move || async move {
        let _ = revision();
        match account() {
            Some(account) => is_stored(account).await.map(Some),
            None => Ok(None),
        }
    });
    let mut draft = use_signal(String::new);
    let mut replacing = use_signal(|| false);
    let mut failure = use_signal(|| None::<String>);

    let mut settle = move |result: Result<(), String>| match result {
        Ok(()) => {
            draft.set(String::new());
            replacing.set(false);
            failure.set(None);
            revision += 1;
            on_change.call(());
        }
        Err(reason) => failure.set(Some(reason)),
    };
    let save = move |_| {
        let Some(account) = account() else { return };
        let key = draft().trim().to_string();
        spawn(async move { settle(store_key(account, key).await) });
    };
    let remove = move |_| {
        let Some(account) = account() else { return };
        spawn(async move { settle(forget_key(account).await) });
    };

    let Some(for_account) = account() else {
        return rsx! {
            div {
                class: "lens-field",
                span { class: "lens-field-label", {t!("preferences.lenses.api_key.label").to_string()} }
                span { class: "lens-field-hint", {t!("preferences.lenses.api_key.no_endpoint").to_string()} }
            }
        };
    };
    let is_stored = matches!(&*stored.read(), Some(Ok(Some(true))));
    let asking = stored.read().is_none();
    let read_error = match &*stored.read() {
        Some(Err(reason)) => Some(reason.clone()),
        _ => None,
    };

    rsx! {
        div {
            class: "lens-field",
            span { class: "lens-field-label", {t!("preferences.lenses.api_key.label").to_string()} }
            if is_stored && !replacing() {
                div {
                    class: "lens-key-line",
                    span { class: "lens-key-stored", {t!("preferences.lenses.api_key.stored", account = for_account).to_string()} }
                    button {
                        class: "lens-button",
                        r#type: "button",
                        onclick: move |_| replacing.set(true),
                        {t!("preferences.lenses.api_key.replace").to_string()}
                    }
                    button {
                        class: "lens-button lens-button-danger",
                        r#type: "button",
                        onclick: remove,
                        {t!("preferences.lenses.api_key.remove").to_string()}
                    }
                }
            } else if !asking {
                div {
                    class: "lens-key-line",
                    input {
                        class: "lens-input monospace",
                        r#type: "password",
                        autocomplete: "off",
                        placeholder: t!("preferences.lenses.api_key.placeholder").to_string(),
                        value: "{draft}",
                        oninput: move |event| draft.set(event.value()),
                    }
                    if !draft().trim().is_empty() {
                        button {
                            class: "lens-button",
                            r#type: "button",
                            onclick: save,
                            {t!("preferences.lenses.api_key.save").to_string()}
                        }
                    }
                    if replacing() {
                        button {
                            class: "lens-button",
                            r#type: "button",
                            onclick: move |_| {
                                draft.set(String::new());
                                replacing.set(false);
                            },
                            {t!("preferences.lenses.cancel").to_string()}
                        }
                    }
                }
            }
            span {
                class: "lens-field-hint",
                if overridden {
                    {t!("preferences.lenses.api_key.overridden").to_string()}
                } else {
                    {t!("preferences.lenses.api_key.hint").to_string()}
                }
            }
            if let Some(reason) = failure().or(read_error) {
                span { class: "lens-field-hint lens-field-error", "{reason}" }
            }
        }
    }
}

/// A program and its arguments, one per line.
///
/// The text is kept as typed, apart from what it means: the arguments are
/// trimmed and blank lines dropped, and writing that back into the field on
/// every keystroke would take away the space or the new line being typed.
#[component]
pub(super) fn ArgvField(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: ReadSignal<Vec<String>>,
    rows: u32,
    on_change: EventHandler<Vec<String>>,
) -> Element {
    let mut draft = use_signal(|| value.peek().join("\n"));
    // A value that is not what the draft means came from elsewhere — the
    // file edited by hand while this is open — and replaces the draft;
    // typing on over a stale draft would write that edit away.
    use_effect(move || {
        let value = value();
        if argv(&draft.peek()) != value {
            draft.set(value.join("\n"));
        }
    });

    rsx! {
        label {
            class: "lens-field",
            span { class: "lens-field-label", "{label}" }
            textarea {
                class: "lens-input monospace",
                rows: "{rows}",
                spellcheck: false,
                value: "{draft}",
                oninput: move |event| {
                    let text = event.value();
                    on_change.call(argv(&text));
                    draft.set(text);
                },
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}

#[component]
pub(super) fn NumberField(
    #[props(into)] label: String,
    #[props(into)] hint: String,
    value: String,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "lens-field lens-field-number",
            span { class: "lens-field-label", "{label}" }
            input {
                class: "lens-input",
                r#type: "number",
                min: "0",
                value: "{value}",
                oninput: move |event| on_input.call(event.value()),
            }
            span { class: "lens-field-hint", "{hint}" }
        }
    }
}
