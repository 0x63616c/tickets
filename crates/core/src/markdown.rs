//! The only HTML rendering boundary for ticket descriptions and comments.
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};

pub fn render(markdown: &str) -> String {
    let mut output = String::new();
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    // Each Markdown body is a section beneath the page title. Compress skipped
    // source levels while preserving nesting, independent of visual styling.
    let mut ancestors: Vec<(HeadingLevel, HeadingLevel)> = Vec::new();
    let mut current = HeadingLevel::H2;
    let events = Parser::new_ext(markdown, options).map(|event| match event {
        Event::Start(Tag::Heading { level, .. }) => {
            while ancestors.last().is_some_and(|(source, _)| *source >= level) {
                ancestors.pop();
            }
            current = match ancestors.last().map(|(_, mapped)| mapped) {
                None => HeadingLevel::H2,
                Some(HeadingLevel::H2) => HeadingLevel::H3,
                Some(HeadingLevel::H3) => HeadingLevel::H4,
                Some(HeadingLevel::H4) => HeadingLevel::H5,
                _ => HeadingLevel::H6,
            };
            ancestors.push((level, current));
            Event::Start(Tag::Heading {
                level: current,
                id: None,
                classes: vec!["markdown-heading".into()],
                attrs: Vec::new(),
            })
        }
        Event::End(TagEnd::Heading(_)) => Event::End(TagEnd::Heading(current)),
        event => event,
    });
    html::push_html(&mut output, events);
    ammonia::Builder::default()
        .add_tags(&["input"])
        .add_allowed_classes("h2", &["markdown-heading"])
        .add_allowed_classes("h3", &["markdown-heading"])
        .add_allowed_classes("h4", &["markdown-heading"])
        .add_allowed_classes("h5", &["markdown-heading"])
        .add_allowed_classes("h6", &["markdown-heading"])
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
    fn headings_fit_beneath_the_page_title() {
        let html = render("### First\n\n###### Child\n\n## Sibling\n\n# Root");
        assert_eq!(
            html,
            "<h2 class=\"markdown-heading\">First</h2>\n<h3 class=\"markdown-heading\">Child</h3>\n<h2 class=\"markdown-heading\">Sibling</h2>\n<h2 class=\"markdown-heading\">Root</h2>\n"
        );
    }
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
