//! Showing a document while it is still being written.
//!
//! The command writes the answer from the top down, so the part already
//! written is final — except the block it is in the middle of. Rendering
//! that block half-written would show a table without its rows or a list
//! item cut mid-word, and then change it under the reader a moment later.
//! Only what ends at a blank line is shown, and never from inside a fenced
//! code block, where a blank line ends nothing.

/// The part of `text` that is made of finished blocks.
pub(crate) fn finished_blocks(text: &str) -> &str {
    let mut fence: Option<(char, usize)> = None;
    let mut finished = 0;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        offset += line.len();
        if !line.ends_with('\n') {
            break;
        }
        let trimmed = line.trim_start_matches([' ', '\t', '>']);
        let run = |marker: char| trimmed.chars().take_while(|&c| c == marker).count();
        match fence {
            Some((marker, length)) => {
                if run(marker) >= length && trimmed[run(marker)..].trim().is_empty() {
                    fence = None;
                }
            }
            None => {
                if let Some(marker) = ['`', '~'].into_iter().find(|&marker| run(marker) >= 3) {
                    fence = Some((marker, run(marker)));
                } else if line.trim().is_empty() {
                    finished = offset;
                }
            }
        }
    }
    &text[..finished]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_is_shown_once_a_blank_line_follows_it() {
        assert_eq!(finished_blocks("# Title\n\nHalf a para"), "# Title\n\n");
        assert_eq!(
            finished_blocks("# Title\n\nWhole.\n\n"),
            "# Title\n\nWhole.\n\n"
        );
    }

    #[test]
    fn nothing_is_shown_before_the_first_block_ends() {
        assert_eq!(finished_blocks("# Tit"), "");
        assert_eq!(finished_blocks("# Title\n"), "");
    }

    #[test]
    fn a_blank_line_inside_a_fence_ends_nothing() {
        let text = "Intro.\n\n```\nfn a() {}\n\nfn b() {}\n";
        assert_eq!(finished_blocks(text), "Intro.\n\n");

        let closed = "Intro.\n\n```\nfn a() {}\n\n```\n\nAfter";
        assert_eq!(
            finished_blocks(closed),
            "Intro.\n\n```\nfn a() {}\n\n```\n\n"
        );
    }

    #[test]
    fn a_shorter_fence_does_not_close_a_longer_one() {
        let text = "````\n```\n\n````\n\n";
        assert_eq!(finished_blocks(text), text);
        assert_eq!(finished_blocks("````\n```\n\n"), "");
    }
}
