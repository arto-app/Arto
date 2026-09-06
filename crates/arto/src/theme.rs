pub use dioxus_sdk_window::theme::Theme as DioxusTheme;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

impl From<&str> for Theme {
    fn from(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::Auto,
        }
    }
}

pub fn resolve_theme(theme: Theme) -> DioxusTheme {
    match theme {
        // NOTE:
        // We cannot use dioxus_sdk_window::theme::get_theme here because
        // it requires a Dioxus runtime and cannot be called from outside
        // of Dioxus context. That's why we use dark_light crate instead.
        // On Linux, dark_light uses D-Bus (zbus) which requires a Tokio
        // runtime. This function may be called before the runtime starts
        // (e.g. from build_custom_index in main()), so catch_unwind
        // prevents the panic and falls back to Light.
        Theme::Auto => match std::panic::catch_unwind(dark_light::detect)
            .ok()
            .and_then(|r| r.ok())
        {
            Some(dark_light::Mode::Light) => DioxusTheme::Light,
            Some(dark_light::Mode::Dark) => DioxusTheme::Dark,
            Some(dark_light::Mode::Unspecified) | None => DioxusTheme::Light,
        },
        Theme::Light => DioxusTheme::Light,
        Theme::Dark => DioxusTheme::Dark,
    }
}
