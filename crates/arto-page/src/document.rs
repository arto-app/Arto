//! The HTML document a page is: the rendered body, the stylesheet and the
//! scripts it calls for, inline, under a policy that runs nothing else.

use crate::css::{self, STANDALONE_OVERRIDE_CSS};
use crate::{PageOptions, Theme};
use arto_frontend::Libraries;
use base64::Engine;
use sha2::{Digest, Sha256};

/// The frontend bundle (IIFE build exposing `window.ArtoRenderer`), embedded
/// at compile time.
const PAGE_JS: &str = include_str!("../assets/frontend/page.iife.js");

/// The inline bootstrap script. Settles the theme (the frontend reads
/// `data-theme` on the root element during initialization) and then starts the
/// frontend.
///
/// The theme choices travel in `data-theme-preference`, `data-light-theme`
/// and `data-dark-theme` on `<html>` rather than in this script, so the script
/// stays byte-identical across pages and its CSP hash can be a constant. The
/// preference `light` and `dark` are taken as-is; anything else follows
/// `prefers-color-scheme`. The mode then picks one of the two theme names.
const BOOTSTRAP_JS: &str = r#"(function(){
  try {
    var root = document.documentElement;
    var preference = root.getAttribute('data-theme-preference');
    var dark = preference === 'dark';
    if (preference !== 'light' && preference !== 'dark') {
      dark = !!(window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
    }
    var theme = root.getAttribute(dark ? 'data-dark-theme' : 'data-light-theme');
    root.setAttribute('data-theme', theme || (dark ? 'dark' : 'light'));
  } catch (e) {}
  if (window.ArtoRenderer && typeof window.ArtoRenderer.init === 'function') { window.ArtoRenderer.init(); }
})();"#;

/// Base64-encoded SHA-256 digest, as required by a CSP `'sha256-...'` source.
fn sha256_base64(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(digest)
}

/// Assemble a full, self-contained HTML document from a rendered Markdown body.
///
/// The embedded stylesheet and frontend bundle are inlined so the page needs no
/// network or filesystem access. With [`PageOptions::content_security_policy`]
/// on, a `Content-Security-Policy` restricts script execution to the two
/// first-party inline scripts (by SHA-256 hash), so raw HTML in an untrusted
/// Markdown body cannot run JavaScript.
pub(crate) fn build_document(body_html: &str, options: &PageOptions) -> String {
    let libraries = Libraries::for_fragment(body_html);

    // The page runtime, then the libraries this document calls for, then the
    // bootstrap. The order is the dependency: the runtime publishes the slots
    // a library registers itself in, and the bootstrap starts a page that has
    // everything it is going to get.
    let scripts: Vec<String> = std::iter::once(PAGE_JS)
        .chain(libraries.scripts())
        .chain(std::iter::once(BOOTSTRAP_JS))
        .map(inline_script)
        .collect();

    // Allowlist exactly the inline scripts we emit; everything else the
    // Markdown body may contain (script tags, event handlers, javascript: URLs)
    // is blocked. `style-src 'unsafe-inline'` is required because the page
    // carries its stylesheet inline and the frontend sets inline styles at
    // runtime.
    let csp_meta = if options.content_security_policy {
        let sources: Vec<String> = scripts
            .iter()
            .map(|script| format!("'sha256-{}'", sha256_base64(script)))
            .collect();
        format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; \
             script-src {sources}; \
             style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; base-uri 'none'\">\n",
            sources = sources.join(" "),
        )
    } else {
        String::new()
    };

    // `data-theme` is what the stylesheet reads, so a fixed theme applies
    // before any script runs (and with scripts blocked). `Auto` starts light
    // and lets the bootstrap consult `prefers-color-scheme`.
    let light_theme = options.light_theme.as_str();
    let dark_theme = options.dark_theme.as_str();
    let (initial_theme, theme_preference) = match options.theme {
        Theme::Auto => (light_theme, "auto"),
        Theme::Light => (light_theme, "light"),
        Theme::Dark => (dark_theme, "dark"),
    };

    // Both passes drop what this page can never paint with. Math is typeset in
    // the page rather than in the HTML handed to it, so a body with no
    // `preprocessed-math*` container never reaches for a KaTeX glyph; and the
    // stylesheet's other themes have no name left that could select them.
    let css = css::stylesheet(libraries.math, light_theme, dark_theme);

    let scripts = scripts
        .iter()
        .map(|script| format!("<script>{script}</script>"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<!DOCTYPE html><html data-theme="{initial_theme}" data-theme-preference="{theme_preference}" data-light-theme="{light_theme}" data-dark-theme="{dark_theme}"><head><meta charset="utf-8">
{csp_meta}<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{css}</style>
<style>{standalone_override}</style>
<style>.markdown-viewer{{{typography}}}</style></head>
<body>
<div class="markdown-viewer"><article class="markdown-body">{body}</article></div>
{scripts}
</body></html>"#,
        csp_meta = csp_meta,
        css = css,
        standalone_override = STANDALONE_OVERRIDE_CSS,
        typography = options.typography.css_declarations(),
        initial_theme = initial_theme,
        theme_preference = theme_preference,
        light_theme = light_theme,
        dark_theme = dark_theme,
        body = body_html,
        scripts = scripts,
    )
}

/// A script as it can be carried in an inline `<script>`.
///
/// Guards against premature termination: if a minified bundle ever contains
/// the literal `</script` (only possible inside a JS string or regex, where
/// `<\/script` is equivalent), the HTML parser would close the inline script
/// early and `ArtoRenderer` would never be defined.
fn inline_script(script: &str) -> String {
    script.replace("</script", r"<\/script")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CjkFontLanguage, ColorTheme, FontFamilyChoice, TypographyConfig};

    #[test]
    fn test_build_document_wraps_body_and_embeds_assets() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        // Body is wrapped in the markdown-viewer container (which supplies the
        // app's padding, background, and centered max-width) and article.
        assert!(html.contains(
            r#"<div class="markdown-viewer"><article class="markdown-body"><p>hi</p></article></div>"#
        ));
        // Frontend is bootstrapped.
        assert!(html.contains("ArtoRenderer.init"));
        // Styles and scripts are inlined.
        assert!(html.contains("<style>"));
        assert!(html.contains("</style>"));
        // A standalone document has no inner `.content` scroll container, so
        // the shared `body { overflow: hidden }` must be overridden or the
        // page cannot scroll long documents.
        assert!(html.contains(STANDALONE_OVERRIDE_CSS));
        assert!(html.contains("html{overflow:auto"));
        // The body gives its scrolling to the viewport rather than becoming a
        // scroll container of its own that never scrolls: a sticky table
        // header holds against the nearest one, and would never move.
        assert!(html.contains("body{overflow:visible"));
        assert!(html.contains("<script>"));
        assert!(html.contains("</script>"));
        // Theme defaults are present: start light, let the bootstrap follow
        // the system.
        assert!(html.contains(r#"<html data-theme="light" data-theme-preference="auto" data-light-theme="light" data-dark-theme="dark">"#));
        assert!(html.contains("prefers-color-scheme"));
    }

    #[test]
    fn test_build_document_embeds_the_math_fonts_only_for_a_document_with_math() {
        let plain = build_document("<p>hi</p>", &PageOptions::default());
        let math = build_document(
            r#"<span class="preprocessed-math-inline" data-original-content="x">x</span>"#,
            &PageOptions::default(),
        );

        // The KaTeX faces carry their woff2 inline, so leaving them out of a
        // document that sets no math saves megabytes.
        assert!(math.len() > plain.len() + 1_000_000);
        assert!(math.contains("@font-face"));
        // Both are still whole pages.
        assert!(plain.contains("ArtoRenderer.init"));
        assert!(math.contains("ArtoRenderer.init"));
    }

    /// A body that draws one diagram and nothing else.
    const DIAGRAM_BODY: &str =
        r#"<pre class="preprocessed-mermaid" data-original-content="graph TD;">graph TD;</pre>"#;

    /// A body that sets one formula and nothing else.
    const MATH_BODY: &str =
        r#"<span class="preprocessed-math-inline" data-original-content="x">x</span>"#;

    #[test]
    fn test_build_document_embeds_mermaid_only_for_a_document_with_a_diagram() {
        let plain = build_document("<p>hi</p>", &PageOptions::default());
        let diagram = build_document(DIAGRAM_BODY, &PageOptions::default());

        // Pele is several times the page runtime by weight, so what a prose
        // page saves is most of its scripts.
        assert!(diagram.len() > plain.len() + 500_000);
        assert_eq!(plain.matches("<script>").count(), 2);
        assert_eq!(diagram.matches("<script>").count(), 3);
    }

    #[test]
    fn test_build_document_embeds_katex_only_for_a_document_with_math() {
        let plain = build_document("<p>hi</p>", &PageOptions::default());
        let math = build_document(MATH_BODY, &PageOptions::default());

        assert_eq!(plain.matches("<script>").count(), 2);
        assert_eq!(math.matches("<script>").count(), 3);
    }

    #[test]
    fn test_build_document_embeds_both_libraries_for_a_document_that_has_both() {
        let html = build_document(
            &format!("{DIAGRAM_BODY}{MATH_BODY}"),
            &PageOptions::default(),
        );

        assert_eq!(html.matches("<script>").count(), 4);
    }

    #[test]
    fn test_build_document_carries_mermaid_for_a_fenced_block_too() {
        // The container above is what `arto-markdown` writes, but the
        // renderer draws a plain fenced block as well, and a page that
        // carried no Mermaid would leave it as source.
        let html = build_document(
            r#"<pre><code class="language-mermaid">graph TD;</code></pre>"#,
            &PageOptions::default(),
        );

        assert_eq!(html.matches("<script>").count(), 3);
    }

    #[test]
    fn test_build_document_allowlists_every_script_it_embeds() {
        let html = build_document(DIAGRAM_BODY, &PageOptions::default());

        // A script the policy does not name is a script the page refuses to
        // run, so the count has to follow what was embedded.
        assert_eq!(html.matches("'sha256-").count(), 3);
        assert!(html.contains(&format!(
            "'sha256-{}'",
            sha256_base64(&inline_script(arto_frontend::MERMAID))
        )));
    }

    #[test]
    fn test_build_document_carries_only_the_themes_the_page_can_paint() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        assert!(html.contains("[data-theme=light]{"));
        assert!(html.contains("[data-theme=dark]{"));
        assert!(!html.contains("[data-theme=dark_dimmed]{"));
        assert!(!html.contains("[data-theme=light_high_contrast]{"));
    }

    #[test]
    fn test_build_document_keeps_the_themes_the_options_name() {
        let html = build_document(
            "<p>hi</p>",
            &PageOptions {
                light_theme: ColorTheme::LightHighContrast,
                dark_theme: ColorTheme::DarkDimmed,
                ..PageOptions::default()
            },
        );

        assert!(html.contains("[data-theme=light_high_contrast]{"));
        assert!(html.contains("[data-theme=dark_dimmed]{"));
        assert!(!html.contains("[data-theme=light]{"));
        assert!(!html.contains("[data-theme=dark]{"));
    }

    #[test]
    fn test_build_document_has_no_premature_script_termination() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        // The only `</script` occurrences must be our own closing tags, i.e.
        // the embedded bundle must not smuggle in a `</script` that would
        // close the inline script early.
        assert_eq!(html.matches("</script>").count(), 2);
        assert_eq!(html.matches("</script").count(), 2);

        // The libraries are embedded the same way and have to hold to the
        // same rule; Pele is among the largest of them.
        let diagram = build_document(DIAGRAM_BODY, &PageOptions::default());
        assert_eq!(diagram.matches("</script>").count(), 3);
        assert_eq!(diagram.matches("</script").count(), 3);
    }

    #[test]
    fn test_build_document_has_csp_allowlisting_only_inline_scripts() {
        let html = build_document("<p>hi</p>", &PageOptions::default());

        // The page carries a CSP that allowlists scripts by hash (no
        // 'unsafe-inline'), so injected <script>/event handlers cannot run.
        assert!(html.contains(r#"http-equiv="Content-Security-Policy""#));
        assert!(html.contains("script-src 'sha256-"));
        assert!(!html.contains("'unsafe-inline'; script"));
        // The hashes must match the exact inline script contents we emit.
        assert!(html.contains(&format!(
            "'sha256-{}'",
            sha256_base64(&inline_script(PAGE_JS))
        )));
        assert!(html.contains(&format!("'sha256-{}'", sha256_base64(BOOTSTRAP_JS))));
    }

    #[test]
    fn test_build_document_can_omit_csp() {
        let options = PageOptions {
            content_security_policy: false,
            ..PageOptions::default()
        };
        let html = build_document("<p>hi</p>", &options);

        assert!(!html.contains("Content-Security-Policy"));
        // The rest of the document is unchanged.
        assert!(html.contains("ArtoRenderer.init"));
        assert!(html.contains(r#"<meta name="viewport""#));
    }

    /// The rule that declares the typography, apart from the stylesheet,
    /// which names the same properties to read them.
    fn typography_rule(html: &str) -> &str {
        let start = html
            .find("<style>.markdown-viewer{")
            .expect("the typography rule");
        let end = start + html[start..].find("</style>").unwrap();
        &html[start..end]
    }

    #[test]
    fn test_build_document_sets_the_text_as_configured() {
        let html = build_document("<p>hi</p>", &PageOptions::default());
        assert!(html.contains(
            "<style>.markdown-viewer{--reading-measure: 60em; --reading-line-height: 1.5; --reading-font-size: 16px; --reading-font-family: initial;}</style>"
        ));

        let serif = build_document(
            "<p>hi</p>",
            &PageOptions {
                typography: TypographyConfig {
                    measure: 40.0,
                    font_family: FontFamilyChoice::Serif,
                    ..TypographyConfig::default()
                },
                ..PageOptions::default()
            },
        );
        assert!(typography_rule(&serif).contains("--reading-measure: 40em;"));
        assert!(typography_rule(&serif).contains("--reading-font-family: \"Iowan Old Style\""));
    }

    #[test]
    fn test_build_document_draws_cjk_text_in_the_chosen_language_faces_only() {
        let chinese = build_document(
            "<p>hi</p>",
            &PageOptions {
                typography: TypographyConfig {
                    cjk_font_language: CjkFontLanguage::ZhHans,
                    ..TypographyConfig::default()
                },
                ..PageOptions::default()
            },
        );
        assert!(typography_rule(&chinese).contains(r#""PingFang SC""#));
        // It picks faces only: the page does not claim a language.
        assert!(chinese.starts_with("<!DOCTYPE html><html data-theme="));
    }

    #[test]
    fn test_build_document_does_not_embed_a_custom_face_that_leaves_its_rule() {
        let html = build_document(
            "<p>hi</p>",
            &PageOptions {
                typography: TypographyConfig {
                    font_family: FontFamilyChoice::Custom,
                    custom_font_family: "serif}</style><script>alert(1)</script>".to_string(),
                    ..TypographyConfig::default()
                },
                ..PageOptions::default()
            },
        );
        assert!(!html.contains("alert(1)"));
        assert!(typography_rule(&html).contains("--reading-font-family: initial;"));
    }

    #[test]
    fn test_build_document_applies_a_fixed_theme_before_scripts_run() {
        let dark = build_document(
            "<p>hi</p>",
            &PageOptions {
                theme: Theme::Dark,
                ..PageOptions::default()
            },
        );
        assert!(dark.contains(r#"<html data-theme="dark" data-theme-preference="dark" data-light-theme="light" data-dark-theme="dark">"#));

        let light = build_document(
            "<p>hi</p>",
            &PageOptions {
                theme: Theme::Light,
                ..PageOptions::default()
            },
        );
        assert!(light.contains(r#"<html data-theme="light" data-theme-preference="light" data-light-theme="light" data-dark-theme="dark">"#));

        // The bootstrap is byte-identical whatever the theme, so its CSP hash
        // stays valid.
        let hash = format!("'sha256-{}'", sha256_base64(BOOTSTRAP_JS));
        assert!(dark.contains(&hash));
        assert!(light.contains(&hash));
    }
}
