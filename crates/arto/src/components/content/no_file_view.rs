use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};

#[component]
pub fn NoFileView() -> Element {
    rsx! {
        div {
            class: "no-file",
            div {
                class: "no-file-container",
                div {
                    class: "no-file-icon",
                    Icon { name: IconName::File, size: 64 }
                }
                h2 {
                    class: "no-file-title",
                    "No File Opened"
                }
                p {
                    class: "no-file-description",
                    "Open a markdown file to view its beautifully rendered content with full support for code highlighting, diagrams, and mathematics."
                }
                div {
                    class: "no-file-hints",
                    div {
                        class: "no-file-hint",
                        span {
                            class: "no-file-hint-icon",
                            Icon { name: IconName::FolderOpen, size: 20 }
                        }
                        span { class: "no-file-hint-text", "Drag and drop a Markdown file or directory here" }
                    }
                    div {
                        class: "no-file-hint",
                        span {
                            class: "no-file-hint-icon",
                            Icon { name: IconName::Command, size: 20 }
                        }
                        span {
                            class: "no-file-hint-text",
                            "Use "
                            kbd {
                                class: "no-file-hint-kbd",
                                "Cmd+O"
                            }
                            " to open a file"
                        }
                    }
                    div {
                        class: "no-file-hint",
                        span {
                            class: "no-file-hint-icon",
                            Icon { name: IconName::Click, size: 20 }
                        }
                        span { class: "no-file-hint-text", "Right-click in Finder and choose \"Open with Arto\"" }
                    }
                }
            }
        }
    }
}
