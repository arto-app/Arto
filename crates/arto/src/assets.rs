use dioxus::asset_resolver::asset_path;
use dioxus::prelude::*;

pub static MAIN_SCRIPT: Asset = asset!("/assets/dist/main.js");
pub static MAIN_STYLE: Asset = asset!("/assets/dist/main.css");

static ARTO_HEADER_IMAGE: Asset = asset!("/assets/arto-header-welcome.png");
static WELCOME_TEMPLATE: Asset = asset!("/assets/welcome.md");

// Embed and process default markdown content at runtime
pub fn get_default_markdown_content() -> String {
    let template_path = asset_path(WELCOME_TEMPLATE).expect("Failed to resolve WELCOME_TEMPLATE");
    let template = std::fs::read_to_string(template_path).expect("Failed to read WELCOME_TEMPLATE");

    let header_path = asset_path(ARTO_HEADER_IMAGE).expect("Failed to resolve ARTO_HEADER_IMAGE");
    let header_str = header_path
        .to_str()
        .expect("Failed to convert ARTO_HEADER_IMAGE path to str");

    // Replace relative image path with data URL
    //template.replace("../assets/arto-header-welcome.png", &header_data_url)
    template.replace("../assets/arto-header-welcome.png", header_str)
}
