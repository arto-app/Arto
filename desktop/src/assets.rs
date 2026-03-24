use dioxus::asset_resolver::asset_path;
use dioxus::prelude::*;
pub static MAIN_SCRIPT: Asset = asset!("/assets/dist/main.js");
pub static MAIN_STYLE: Asset = asset!("/assets/dist/main.css");

#[cfg(windows)]
pub fn windows_safe_asset_url(asset: &dioxus::prelude::Asset) -> String {
    let mut path = asset.to_string().replace('\\', "/");
    if !path.starts_with('/') && !path.starts_with("http") {
        path = format!("/{}", path);
    }
    path
}

pub fn get_main_script_path() -> String {
    #[cfg(windows)]
    return windows_safe_asset_url(&MAIN_SCRIPT);
    #[cfg(not(windows))]
    return MAIN_SCRIPT.to_string();
}

pub fn get_main_style_path() -> String {
    #[cfg(windows)]
    return windows_safe_asset_url(&MAIN_STYLE);
    #[cfg(not(windows))]
    return MAIN_STYLE.to_string();
}

static ARTO_HEADER_IMAGE: Asset = asset!("/assets/arto-header-welcome.png");
static WELCOME_TEMPLATE: Asset = asset!("/assets/welcome.md");

// Embed and process default markdown content at runtime
pub fn get_default_markdown_content() -> String {
    let template_path = asset_path(WELCOME_TEMPLATE).expect("Failed to resolve WELCOME_TEMPLATE");
    let template = std::fs::read_to_string(template_path).expect("Failed to read WELCOME_TEMPLATE");

    let header_path = asset_path(ARTO_HEADER_IMAGE).expect("Failed to resolve ARTO_HEADER_IMAGE");
    let header_str = header_path
        .to_str()
        .expect("Failed to convert ARTO_HEADER_IMAGE path to str")
        .replace('\\', "/");

    let final_header_str = if cfg!(windows) && !header_str.starts_with('/') {
        format!("/{}", header_str)
    } else {
        header_str
    };

    // Replace relative image path with data URL
    //template.replace("../assets/arto-header-welcome.png", &header_data_url)
    template.replace("../assets/arto-header-welcome.png", &final_header_str)
}
