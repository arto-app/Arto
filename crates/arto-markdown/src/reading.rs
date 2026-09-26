//! How much there is to read in a document, block by block.
//!
//! The profile holds counts, not times: how fast a reader reads is theirs to
//! set, and changing it should not mean rendering the document again. What a
//! count turns into is the host's business.

/// What one top-level block of the document asks a reader to read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReadingBlock {
    /// 1-based line of the file the block starts on — the start line of its
    /// `data-source-range`, so the line a scroll anchor names finds it.
    pub line: u32,
    /// Characters of scripts read a character at a time: Han, kana, Hangul
    /// and full-width letters and digits.
    pub cjk_chars: u32,
    /// Words of every other script.
    pub words: u32,
    /// Content lines of code blocks, which are scanned rather than read.
    pub code_lines: u32,
    /// Images, diagrams and display formulas, each looked at as a whole.
    pub figures: u32,
}

/// The reading load of a document, one entry per top-level block, in
/// ascending line order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadingProfile {
    pub blocks: Vec<ReadingBlock>,
}

/// Count `text` as `(cjk_chars, words)`.
///
/// A CJK character is read one at a time, so each counts; any other script
/// is read a word at a time, where a word is a run of characters between
/// whitespace and CJK characters that holds at least one letter or digit.
/// Punctuation, symbols and emoji on their own are not read, so they count
/// for nothing: `Rustの所有権` is one word and four characters.
pub fn count_text(text: impl AsRef<str>) -> (u32, u32) {
    let mut cjk_chars = 0u32;
    let mut words = 0u32;
    let mut in_word = false;
    let mut word_has_alphanumeric = false;
    let mut end_word = |in_word: &mut bool, has_alphanumeric: &mut bool| {
        if *in_word && *has_alphanumeric {
            words += 1;
        }
        *in_word = false;
        *has_alphanumeric = false;
    };
    for c in text.as_ref().chars() {
        if is_cjk(c) {
            end_word(&mut in_word, &mut word_has_alphanumeric);
            cjk_chars += 1;
        } else if c.is_whitespace() {
            end_word(&mut in_word, &mut word_has_alphanumeric);
        } else {
            in_word = true;
            word_has_alphanumeric |= c.is_alphanumeric();
        }
    }
    end_word(&mut in_word, &mut word_has_alphanumeric);
    (cjk_chars, words)
}

/// Whether `c` is read a character at a time.
fn is_cjk(c: char) -> bool {
    matches!(
        c,
        '\u{3005}'                    // 々
        | '\u{3040}'..='\u{309F}'     // Hiragana
        | '\u{30A0}'..='\u{30FF}'     // Katakana
        | '\u{31F0}'..='\u{31FF}'     // Katakana phonetic extensions
        | '\u{3400}'..='\u{4DBF}'     // CJK extension A
        | '\u{4E00}'..='\u{9FFF}'     // CJK unified ideographs
        | '\u{F900}'..='\u{FAFF}'     // CJK compatibility ideographs
        | '\u{1100}'..='\u{11FF}'     // Hangul Jamo
        | '\u{3130}'..='\u{318F}'     // Hangul compatibility Jamo
        | '\u{AC00}'..='\u{D7AF}'     // Hangul syllables
        | '\u{FF10}'..='\u{FF19}'     // Full-width digits
        | '\u{FF21}'..='\u{FF3A}'     // Full-width upper case
        | '\u{FF41}'..='\u{FF5A}'     // Full-width lower case
        | '\u{FF66}'..='\u{FF9D}'     // Half-width katakana
        | '\u{20000}'..='\u{3134F}'   // CJK extensions B and later
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_counts_characters_without_punctuation() {
        assert_eq!(count_text("これは日本語の文章です。"), (11, 0));
        assert_eq!(count_text("「括弧」、読点"), (4, 0));
    }

    #[test]
    fn english_counts_words() {
        assert_eq!(count_text("The quick brown fox jumps."), (0, 5));
        assert_eq!(count_text("don't stop — 3,400 words"), (0, 4));
    }

    #[test]
    fn a_latin_word_inside_japanese_is_one_word() {
        assert_eq!(count_text("Rustの所有権"), (4, 1));
        assert_eq!(count_text("これはOpenAIとAnthropicの話"), (6, 2));
    }

    #[test]
    fn full_width_letters_and_digits_count_as_characters() {
        assert_eq!(count_text("ＡＢＣ１２３"), (6, 0));
    }

    #[test]
    fn kana_and_hangul_count_as_characters() {
        assert_eq!(count_text("ひらがなカタカナｶﾀｶﾅ"), (12, 0));
        assert_eq!(count_text("한국어"), (3, 0));
        assert_eq!(count_text("人々"), (2, 0));
    }

    #[test]
    fn emoji_and_symbols_alone_are_not_read() {
        assert_eq!(count_text("🎉 → * | --- 🚀"), (0, 0));
        assert_eq!(count_text("done 🎉"), (0, 1));
    }

    #[test]
    fn nothing_counts_as_nothing() {
        assert_eq!(count_text(""), (0, 0));
        assert_eq!(count_text("  \n\t "), (0, 0));
    }
}
