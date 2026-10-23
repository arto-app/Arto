use lol_html::errors::RewritingError;
use lol_html::{HtmlRewriter, Settings};

/// A pass that stopped part way through.
#[derive(Debug)]
pub(crate) struct Interrupted {
    pub(crate) error: RewritingError,
    /// What the pass had written before it stopped: every byte of it went
    /// through the handlers, and nothing after it did.
    pub(crate) written: String,
}

/// Run one lol_html pass over `html`.
pub(crate) fn rewrite(html: &str, settings: Settings<'_, '_>) -> Result<String, Interrupted> {
    let mut output = Vec::new();
    let mut rewriter = HtmlRewriter::new(settings, |chunk: &[u8]| {
        output.extend_from_slice(chunk);
    });
    // Not `write(..).and(end())`: the argument is evaluated either way, and a
    // rewriter that has failed panics when it is ended. Dropping it instead
    // releases the borrows `end` would have, the output sink's and the
    // handlers' of whatever they collect into.
    let result = match rewriter.write(html.as_bytes()) {
        Ok(()) => rewriter.end(),
        Err(error) => {
            drop(rewriter);
            Err(error)
        }
    };
    // The input is a `str` and the settings keep lol_html's UTF-8 default, so
    // the output is UTF-8 and the lossy conversion never replaces anything.
    let written = String::from_utf8_lossy(&output).into_owned();
    match result {
        Ok(()) => Ok(written),
        Err(error) => Err(Interrupted { error, written }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lol_html::element;

    #[test]
    fn a_pass_hands_back_the_rewritten_html() {
        let settings = Settings::new().append_element_content_handler(element!("a", |el| {
            el.set_attribute("rel", "noopener")?;
            Ok(())
        }));

        let html = rewrite(r#"<p><a href="x">x</a></p>"#, settings).unwrap();

        assert_eq!(html, r#"<p><a href="x" rel="noopener">x</a></p>"#);
    }

    #[test]
    fn a_failed_pass_hands_back_only_what_went_through_the_handlers() {
        let settings = Settings::new()
            .append_element_content_handler(element!("em", |_| { Err("refused".into()) }));

        let interrupted = rewrite("<p>before</p><em>after</em><p>rest</p>", settings).unwrap_err();

        assert!(matches!(
            interrupted.error,
            RewritingError::ContentHandlerError(_)
        ));
        assert!(
            !interrupted.written.contains("rest"),
            "{}",
            interrupted.written
        );
    }
}
