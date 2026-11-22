//! Files a host answers for
//!
//! A host whose documents do not live on this machine's filesystem — a
//! browser, a phone, a server holding them in a database — hands the
//! renderer the files instead.

use super::*;
use arto_markdown::{FileStat, Files, Renderer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// A folder held in memory, addressed by absolute paths that exist nowhere
/// on disk.
struct MemoryFiles(HashMap<PathBuf, Vec<u8>>);

impl MemoryFiles {
    fn new(files: &[(&str, &[u8])]) -> Self {
        Self(
            files
                .iter()
                .map(|(path, bytes)| (PathBuf::from(path), bytes.to_vec()))
                .collect(),
        )
    }
}

impl Files for MemoryFiles {
    fn stat(&self, path: &Path) -> Option<FileStat> {
        let path = normalize(path);
        let bytes = self.0.get(&path)?;
        Some(FileStat {
            len: bytes.len() as u64,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            path,
        })
    }

    fn read(&self, path: &Path, limit: u64) -> Option<Vec<u8>> {
        let bytes = self.0.get(&normalize(path))?;
        (bytes.len() as u64 <= limit).then(|| bytes.clone())
    }
}

/// `path` with every `.` and `..` taken out, the way the store spells it.
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component),
        }
    }
    normalized
}

#[test]
fn a_link_is_missing_only_when_the_host_has_no_such_file() {
    let files = MemoryFiles::new(&[("/notes/team/other.md", b"# Other")]);

    let html = Renderer::new(&files)
        .render_to_html(
            "[there](./other.md) [gone](../gone.md)\n",
            "/notes/team/doc.md",
            &RenderOptions::default(),
        )
        .expect("renders")
        .html;

    assert!(
        has_element(
            &html,
            "span",
            &[("data-md-link", "./other.md"), ("class", "md-link")]
        ),
        "{html}"
    );
    assert!(
        has_element(
            &html,
            "span",
            &[
                ("data-md-link", "../gone.md"),
                ("class", "md-link md-link-missing")
            ]
        ),
        "{html}"
    );
}

#[test]
fn an_image_is_inlined_from_the_bytes_the_host_holds() {
    let files = MemoryFiles::new(&[("/notes/images/hero.png", &[0x89, 0x50, 0x4E, 0x47])]);

    let html = Renderer::new(&files)
        .render_to_html(
            "![hero](../images/hero.png)\n",
            "/notes/team/doc.md",
            &RenderOptions::default(),
        )
        .expect("renders")
        .html;

    assert!(
        html.contains(r#"src="data:image/png;base64,iVBORw==""#),
        "{html}"
    );
}

#[test]
fn a_deferred_image_names_the_file_the_way_the_host_spells_it() {
    let files = MemoryFiles::new(&[("/notes/images/hero.png", &[0x89, 0x50, 0x4E, 0x47])]);

    let rendered = Renderer::new(&files)
        .render_to_html(
            "![hero](../images/./hero.png)\n",
            "/notes/team/doc.md",
            &RenderOptions {
                images: ImageResolution::Deferred {
                    base_url: "/assets".to_string(),
                },
                ..Default::default()
            },
        )
        .expect("renders");

    assert_eq!(rendered.images.len(), 1, "{:?}", rendered.images);
    assert_eq!(
        rendered.images[0].path,
        PathBuf::from("/notes/images/hero.png")
    );
    assert_eq!(rendered.images[0].mime, "image/png");
    assert!(
        rendered
            .html
            .contains(&format!(r#"src="/assets/{}.png""#, rendered.images[0].id)),
        "{}",
        rendered.html
    );
}

#[test]
fn an_image_the_host_does_not_hold_is_left_as_written() {
    let files = MemoryFiles::new(&[]);

    let html = Renderer::new(&files)
        .render_to_html(
            "![hero](./hero.png)\n",
            "/notes/doc.md",
            &RenderOptions::default(),
        )
        .expect("renders")
        .html;

    assert!(html.contains(r#"src="./hero.png""#), "{html}");
}
