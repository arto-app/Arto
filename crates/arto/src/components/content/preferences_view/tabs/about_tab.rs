use crate::components::icon::{Icon, IconName};
use crate::config::Config;
use crate::utils::file_operations;
use dioxus::prelude::*;

const ARTO_ICON: Asset = asset!("/assets/Arto.png");

#[component]
pub fn AboutTab() -> Element {
    let version_text = format!("Version {}", env!("ARTO_BUILD_VERSION"));
    let config_dir_path = Config::path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let config_dir_text = config_dir_path.display().to_string();

    rsx! {
        div {
            class: "about-page",

            div {
                class: "about-container",

                // Icon
                div {
                    class: "about-icon",
                    img {
                        src: "{ARTO_ICON}",
                        alt: "Arto",
                    }
                }

                // Title
                h2 { class: "about-title", "Arto" }

                // Version
                p { class: "about-version", "{version_text}" }

                // Tagline
                p { class: "about-tagline", "The Art of Reading Markdown." }

                // Description
                p { class: "about-description",
                    "A local app that faithfully recreates GitHub-style Markdown rendering for a beautiful reading experience."
                }

                // Configuration directory
                div {
                    class: "about-config-dir",
                    p { class: "about-config-dir-label", "Configuration Directory" }
                    div {
                        class: "about-config-dir-row",
                        input {
                            class: "about-config-dir-input",
                            r#type: "text",
                            value: "{config_dir_text}",
                            readonly: true,
                        }
                        button {
                            class: "about-config-dir-button",
                            onclick: {
                                let config_dir_path = config_dir_path.clone();
                                move |_| {
                                    file_operations::open_directory_in_finder(&config_dir_path);
                                }
                            },
                            span { class: "about-link-icon", Icon { name: IconName::FolderOpen, size: 18 } }
                            span { class: "about-link-text", "Open in Finder" }
                        }
                    }
                }

                // Links (card style like no-file-hints)
                div {
                    class: "about-links",
                    a {
                        href: "https://github.com/arto-app/Arto",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "about-link",
                        span { class: "about-link-icon", Icon { name: IconName::BrandGithub, size: 20 } }
                        span { class: "about-link-text", "View on GitHub" }
                    }
                    a {
                        href: "https://github.com/arto-app/Arto/issues",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "about-link",
                        span { class: "about-link-icon", Icon { name: IconName::Bug, size: 20 } }
                        span { class: "about-link-text", "Report an Issue" }
                    }
                }

                // Footer
                div {
                    class: "about-footer",
                    p { "Created by lambdalisue" }
                    p { "Copyright © 2025 lambdalisue" }
                }
            }
        }
    }
}
