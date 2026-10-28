//! The only HTML rendering boundary for ticket descriptions and comments.
use pulldown_cmark::{Options, Parser, html};

pub fn render(markdown: &str) -> String {
    let mut output = String::new();
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    html::push_html(&mut output, Parser::new_ext(markdown, options));
    ammonia::Builder::default()
        .add_tags(&["input"])
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .set_tag_attribute_value("input", "type", "checkbox")
        .set_tag_attribute_value("input", "disabled", "")
        .link_rel(Some("noopener noreferrer"))
        .clean(&output)
        .to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gfm() {
        let html = render(
            "- [x] done\n- [ ] todo\n\n~~gone~~\n\n|a|b|\n|-|-|\n|1|2|\n\n```rust\nlet x = 1;\n```\n\n[link](https://example.com)",
        );
        for needle in [
            "<input", "disabled", "checked", "<del>", "<table>", "<code", "noopener",
        ] {
            assert!(html.contains(needle), "{html}");
        }
    }
    #[test]
    fn xss() {
        for payload in [
            "<script>alert(1)</script>",
            "[bad](javascript:alert%281%29)",
            "<img src=x onerror=alert(1)>",
            "<div onclick='alert(1)'>text</div>",
            "<svg><a xlink:href='javascript:alert(1)'>bad</a></svg>",
            "<iframe src='https://example.com'>bad</iframe>",
            "<img src='data:text/html;base64,PHNjcmlwdD4='>",
            "[bad](data:text/html,boom)",
            "<input type=text autofocus onfocus=alert(1)>",
        ] {
            let html = render(payload);
            for forbidden in [
                "<script",
                "javascript:",
                "onerror",
                "onclick",
                "<svg",
                "<iframe",
                "data:",
                "autofocus",
                "onfocus",
                "type=\"text\"",
            ] {
                assert!(!html.contains(forbidden), "{payload}: {html}");
            }
        }
    }
    #[test]
    fn raw_html_keeps_safe_content_but_never_active_controls() {
        let html = render(
            "<strong>Safe</strong><input type=submit formaction='javascript:alert(1)' onclick='alert(1)'><a href='jav&#x61;script:alert(1)'>link</a>",
        );
        assert!(html.contains("<strong>Safe</strong>"));
        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("disabled"));
        for forbidden in ["javascript", "formaction", "onclick", "submit"] {
            assert!(!html.contains(forbidden), "{html}");
        }
    }
}
