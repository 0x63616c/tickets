//! The only HTML rendering boundary for ticket descriptions and comments.
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};

pub fn render(markdown: &str) -> String {
    render_in_project(markdown, None)
}

/// Link bare issue references only in prose, using the current ticket project.
pub fn render_in_project(markdown: &str, project: Option<&str>) -> String {
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
    let mut protected = 0usize;
    let events = events.flat_map(|event| {
        match &event {
            Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_)) => {
                protected += 1
            }
            Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) => {
                protected = protected.saturating_sub(1)
            }
            _ => {}
        }
        if protected == 0
            && let (Some(project), Event::Text(text)) = (project, &event)
        {
            let mut parts = Vec::new();
            let mut start = 0;
            let bytes = text.as_bytes();
            for (at, _) in text.match_indices('#') {
                if at > 0
                    && (bytes[at - 1].is_ascii_alphanumeric()
                        || matches!(bytes[at - 1], b'_' | b'/' | b'&'))
                {
                    continue;
                }
                let end = at
                    + 1
                    + bytes[at + 1..]
                        .iter()
                        .take_while(|b| b.is_ascii_digit())
                        .count();
                if end == at + 1
                    || (end < bytes.len()
                        && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_'))
                {
                    continue;
                }
                if text[at + 1..end]
                    .parse::<u64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .is_none()
                {
                    continue;
                }
                parts.push(Event::Text(text[start..at].to_owned().into()));
                parts.push(Event::Start(Tag::Link {
                    link_type: pulldown_cmark::LinkType::Inline,
                    dest_url: format!("/t/{project}-{}", &text[at + 1..end]).into(),
                    title: "".into(),
                    id: "".into(),
                }));
                parts.push(Event::Text(text[at..end].to_owned().into()));
                parts.push(Event::End(TagEnd::Link));
                start = end;
            }
            if start > 0 {
                parts.push(Event::Text(text[start..].to_owned().into()));
                return parts;
            }
        }
        vec![event]
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
    fn project_references_only_link_prose() {
        let html = render_in_project(
            "See #12 and **#34**. `#56` [#78](https://example.com) word#9 #0 #3word\n\n```text\n#90\n```",
            Some("TIK"),
        );
        assert!(html.contains("href=\"/t/TIK-12\""));
        assert!(html.contains("href=\"/t/TIK-34\""));
        for number in [56, 78, 9, 0, 3, 90] {
            assert!(!html.contains(&format!("/t/TIK-{number}\"")), "{html}");
        }
        assert!(!render("See #12").contains("/t/"));
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
