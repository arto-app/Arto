//! Video attachments GitHub plays where their URL stands.
//!
//! A video dropped into GitHub's editor is written into the document as its
//! bare URL on a line of its own, and github.com shows a player there. That
//! is the site's rendering rather than GFM — the URL names no file type, and
//! GitHub knows the asset is a video only from its own upload records — so no
//! Markdown engine does it, and the rule is read off the URL here: an
//! attachment under `github.com/user-attachments/assets/` written that way is
//! a video, because an image uploaded the same way is written as an `<img>`
//! tag instead. The older upload host serves images too, so a URL there counts
//! only when its extension names a video.

/// The video a paragraph's source text names, when that text is nothing but
/// the URL of a GitHub video attachment, bare or between `<` and `>`.
pub(super) fn attachment(paragraph: &str) -> Option<&str> {
    let text = paragraph.trim();
    let url = text
        .strip_prefix('<')
        .and_then(|text| text.strip_suffix('>'))
        .unwrap_or(text);
    if url.contains(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"')) {
        return None;
    }
    let is_video = if let Some(id) = url.strip_prefix("https://github.com/user-attachments/assets/")
    {
        !id.is_empty() && !id.contains('/')
    } else if let Some(path) = url.strip_prefix("https://user-images.githubusercontent.com/") {
        let extension = path.rsplit_once('.').map(|(_, extension)| extension);
        extension.is_some_and(|extension| {
            ["mp4", "mov", "webm"]
                .iter()
                .any(|video| extension.eq_ignore_ascii_case(video))
        })
    } else {
        false
    };
    is_video.then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attachment_url_is_a_video() {
        let url = "https://github.com/user-attachments/assets/2f1c9e0a";
        assert_eq!(attachment(url), Some(url));
        assert_eq!(attachment(&format!("  <{url}>\n")), Some(url));
    }

    #[test]
    fn anything_else_on_the_line_makes_it_text() {
        assert_eq!(
            attachment("see https://github.com/user-attachments/assets/2f1c9e0a"),
            None
        );
        assert_eq!(
            attachment("https://github.com/user-attachments/assets/"),
            None
        );
        assert_eq!(
            attachment("https://github.com/user-attachments/files/1/report.pdf"),
            None
        );
        assert_eq!(
            attachment("https://github.com/user-attachments/assets/a/b"),
            None
        );
    }

    #[test]
    fn the_older_host_counts_only_a_video_extension() {
        let host = "https://user-images.githubusercontent.com/1";
        for name in ["a.mp4", "a.MOV", "a.webm"] {
            let url = format!("{host}/{name}");
            assert_eq!(attachment(&url), Some(url.as_str()));
        }
        for name in ["a.png", "a.gif", "a"] {
            assert_eq!(attachment(&format!("{host}/{name}")), None);
        }
    }
}
