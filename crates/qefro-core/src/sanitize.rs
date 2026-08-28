//! Server-side HTML sanitization for rich-text fields.
//!
//! Never trust client-generated HTML. The UI may send markup; only a
//! conservative subset is stored and later rendered.

pub fn sanitize_html(input: &str) -> String {
    ammonia::Builder::new()
        .tags(
            [
                "p",
                "br",
                "strong",
                "b",
                "em",
                "i",
                "u",
                "h1",
                "h2",
                "h3",
                "h4",
                "ul",
                "ol",
                "li",
                "a",
                "blockquote",
                "code",
                "pre",
                "span",
            ]
            .into(),
        )
        .url_schemes(["http", "https", "mailto"].into())
        .link_rel(Some("noopener noreferrer"))
        .clean(input)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_and_keeps_formatting() {
        let dirty = r#"<p>Hello <strong>Ada</strong><script>alert(1)</script></p><a href="javascript:alert(1)">x</a>"#;
        let clean = sanitize_html(dirty);
        assert!(clean.contains("<strong>Ada</strong>"));
        assert!(!clean.contains("script"));
        assert!(!clean.contains("javascript:"));
    }
}
