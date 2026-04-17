use std::collections::BTreeMap;

const LIQUID_INCLUDE_PREFIX: &str = "include figure.liquid";

pub(super) fn preprocess_liquid_includes(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());

    for segment in markdown.split_inclusive('\n') {
        let (line, newline) = match segment.strip_suffix('\n') {
            Some(line) => (line, "\n"),
            None => (segment, ""),
        };

        if let Some(rewritten) = rewrite_figure_include(line) {
            output.push_str(&rewritten);
        } else {
            output.push_str(line);
        }
        output.push_str(newline);
    }

    output
}

fn rewrite_figure_include(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("{%")?.strip_suffix("%}")?.trim();
    let attrs = inner.strip_prefix(LIQUID_INCLUDE_PREFIX)?.trim();
    let attrs = parse_attributes(attrs);

    let path = attrs.get("path").or_else(|| attrs.get("src"))?;
    let mut img_attrs = Vec::new();
    img_attrs.push(format!(
        r#"src="{}""#,
        html_escape::encode_double_quoted_attribute(path)
    ));

    for key in ["class", "alt", "title", "width", "height", "loading"] {
        if let Some(value) = attrs.get(key) {
            img_attrs.push(format!(
                r#"{key}="{}""#,
                html_escape::encode_double_quoted_attribute(value)
            ));
        }
    }

    let image_tag = format!("<img {}>", img_attrs.join(" "));
    match attrs.get("caption") {
        Some(caption) => Some(format!(
            "<figure>{}<figcaption>{}</figcaption></figure>",
            image_tag,
            html_escape::encode_text(caption)
        )),
        None => Some(image_tag),
    }
}

fn parse_attributes(input: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }

        let key_start = index;
        while index < chars.len() && !chars[index].is_whitespace() && chars[index] != '=' {
            index += 1;
        }
        if key_start == index {
            index += 1;
            continue;
        }
        let key: String = chars[key_start..index].iter().collect();

        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() || chars[index] != '=' {
            continue;
        }
        index += 1;

        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }

        let quote = chars[index];
        let value = if let Some(closing_quote) = matching_quote(quote) {
            index += 1;
            let value_start = index;
            while index < chars.len() && chars[index] != closing_quote {
                index += 1;
            }
            let value: String = chars[value_start..index].iter().collect();
            if index < chars.len() && chars[index] == closing_quote {
                index += 1;
            }
            value
        } else {
            let value_start = index;
            while index < chars.len() && !chars[index].is_whitespace() {
                index += 1;
            }
            chars[value_start..index].iter().collect()
        };

        attrs.insert(key, value);
    }

    attrs
}

fn matching_quote(ch: char) -> Option<char> {
    match ch {
        '"' => Some('"'),
        '\'' => Some('\''),
        '“' => Some('”'),
        '‘' => Some('’'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_figure_include_with_ascii_quotes() {
        let markdown =
            r#"{% include figure.liquid path="assets/img/example.png" class="img-fluid" %}"#;

        let rewritten = preprocess_liquid_includes(markdown);

        assert_eq!(
            rewritten,
            r#"<img src="assets/img/example.png" class="img-fluid">"#
        );
    }

    #[test]
    fn rewrites_figure_include_with_curly_quotes() {
        let markdown =
            "{% include figure.liquid path=“assets/img/example.png” class=“img-fluid” %}";

        let rewritten = preprocess_liquid_includes(markdown);

        assert_eq!(
            rewritten,
            r#"<img src="assets/img/example.png" class="img-fluid">"#
        );
    }

    #[test]
    fn rewrites_captioned_figure_include() {
        let markdown =
            r#"{% include figure.liquid path="assets/img/example.png" caption="A demo image" %}"#;

        let rewritten = preprocess_liquid_includes(markdown);

        assert_eq!(
            rewritten,
            r#"<figure><img src="assets/img/example.png"><figcaption>A demo image</figcaption></figure>"#
        );
    }

    #[test]
    fn leaves_other_liquid_tags_untouched() {
        let markdown = r#"{% include something-else.liquid path="assets/img/example.png" %}"#;

        assert_eq!(preprocess_liquid_includes(markdown), markdown);
    }
}
