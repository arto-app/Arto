//! Images

use super::*;

/// The `<picture>` shape GitHub documents for theme-aware images. The
/// document has no base URL, so every candidate has to be inlined or the
/// theme that picks the `<source>` shows an empty space.
#[test]
fn a_picture_inlines_both_the_source_and_the_img() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("dark.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();
    std::fs::write(dir.path().join("light.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let html = render_to_html(
        indoc! {r#"
            <picture>
              <source media="(prefers-color-scheme: dark)" srcset="./dark.png">
              <img src="./light.png" alt="hero">
            </picture>
        "#},
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html;

    assert_eq!(html.matches("data:image/png;base64,").count(), 2, "{html}");
    assert!(!html.contains("./dark.png"), "{html}");
}

#[test]
fn a_deferred_image_leaves_the_bytes_out_and_names_the_file() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hero.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let rendered = render_to_html(
        "![hero](./hero.png)\n\n![again](./hero.png)\n",
        dir.path().join("doc.md"),
        &RenderOptions {
            images: ImageResolution::Deferred {
                base_url: "artoasset://localhost/img".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("renders");

    assert!(
        !rendered.html.contains("base64"),
        "the bytes stayed in the document: {}",
        rendered.html
    );
    // One file drawn twice is one entry and one URL, which is what lets the
    // host serve it once and the WebView cache it.
    assert_eq!(rendered.images.len(), 1, "{:?}", rendered.images);
    assert_eq!(
        rendered.images[0].path,
        dir.path().join("hero.png").canonicalize().unwrap()
    );
    let url = format!("artoasset://localhost/img/{}", rendered.images[0].id);
    assert_eq!(rendered.html.matches(&url).count(), 2, "{}", rendered.html);
}

/// Whether an image can be shown decides the markup, so it is decided while
/// the markup is written — the host serving the URL is far too late to drop a
/// candidate the browser has already preferred.
#[test]
fn a_deferred_srcset_drops_the_candidate_it_cannot_serve() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("small.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();
    std::fs::write(dir.path().join("empty.png"), []).unwrap();

    let rendered = render_to_html(
        r#"<img src="./small.png" srcset="./empty.png 2x, ./small.png 1x">"#,
        dir.path().join("doc.md"),
        &RenderOptions {
            images: ImageResolution::Deferred {
                base_url: "artoasset://localhost/img".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("renders");

    assert_eq!(rendered.images.len(), 1, "{:?}", rendered.images);
    assert!(
        rendered
            .images
            .iter()
            .all(|image| image.path.ends_with("small.png")),
        "{:?}",
        rendered.images
    );
    assert!(!rendered.html.contains("empty.png"), "{}", rendered.html);
}

#[test]
fn a_deferred_image_carries_the_type_to_answer_with() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("diagram.svg"), b"<svg/>").unwrap();

    let rendered = render_to_html(
        "![diagram](./diagram.svg)",
        dir.path().join("doc.md"),
        &RenderOptions {
            images: ImageResolution::Deferred {
                base_url: "artoasset://localhost/img".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("renders");

    assert_eq!(rendered.images[0].mime, "image/svg+xml");
}

#[test]
fn an_inlined_image_leaves_nothing_for_the_host_to_serve() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hero.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let rendered = render_to_html(
        "![hero](./hero.png)",
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders");

    assert!(rendered.html.contains("data:image/png;base64,"));
    assert!(rendered.images.is_empty(), "{:?}", rendered.images);
}

#[test]
fn an_image_named_by_a_file_url_is_inlined() {
    let dir = tempfile::TempDir::new().unwrap();
    let image = dir.path().join("hero.png");
    std::fs::write(&image, [0x89, 0x50, 0x4E, 0x47]).unwrap();
    let url = url::Url::from_file_path(image.canonicalize().unwrap()).unwrap();

    let html = render_to_html(
        format!("![hero]({url})"),
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html;

    assert!(html.contains("data:image/png;base64,"), "{html}");
}

#[cfg(unix)]
#[test]
fn an_image_named_by_a_localhost_file_url_is_inlined() {
    let dir = tempfile::TempDir::new().unwrap();
    let image = dir.path().join("hero.png");
    std::fs::write(&image, [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let html = render_to_html(
        format!(
            "![hero](file://localhost{})",
            image.canonicalize().unwrap().display()
        ),
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html;

    assert!(html.contains("data:image/png;base64,"), "{html}");
}

/// A `file:` URL that names another host is no file on this machine, and
/// the image falls back to reading what follows the scheme as a path. On
/// Windows the same URL is a UNC share, which is why this is Unix only.
#[cfg(unix)]
#[test]
fn an_image_named_by_a_file_url_with_a_host_is_read_as_a_path_under_the_document() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("img")).unwrap();
    std::fs::write(dir.path().join("img/hero.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let html = render_to_html(
        "![hero](file://img/hero.png)",
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html;

    assert!(html.contains("data:image/png;base64,"), "{html}");
}

#[test]
fn an_image_whose_name_holds_a_colon_is_inlined() {
    // Unlike a link, an image reference is a path unless it is http(s) or
    // data, so a file name that reads like `scheme:rest` is still a file.
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("shot:1.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let html = render_to_html(
        "![shot](<shot:1.png>)",
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html;

    assert!(html.contains("data:image/png;base64,"), "{html}");
}
