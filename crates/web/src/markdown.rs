/// The only entry point for rendering ticket and comment Markdown as HTML.
pub fn render(source: &str) -> String {
    let options = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_TASKLISTS;
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new_ext(source, options));
    ammonia::Builder::default()
        .add_tags(&["input"])
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .set_tag_attribute_value("input", "type", "checkbox")
        .set_tag_attribute_value("input", "disabled", "")
        .clean(&html)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn renders_gfm_and_removes_active_content() {
        let html = render(
            "## Checks\n- [x] Ready\n\n| Key | Value |\n| --- | --- |\n| a | b |\n\n~~old~~ **new** `code`\n\n<script>alert(1)</script><img src=x onerror=alert(1)>\n\n[bad](javascript:alert%281%29)",
        );
        for expected in [
            "<h2>", "checkbox", "disabled", "<table>", "<del>", "<strong>", "<code>",
        ] {
            assert!(html.contains(expected), "missing {expected}: {html}");
        }
        for unsafe_text in ["<script", "onerror", "javascript:"] {
            assert!(!html.contains(unsafe_text), "unsafe {unsafe_text}: {html}");
        }
    }

    #[test]
    fn raw_form_controls_cannot_become_interactive() {
        let html = render(
            "<input type=submit formaction=//example.com autofocus><iframe src=//example.com></iframe><svg onload=alert(1)></svg>",
        );
        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("disabled"));
        for unsafe_text in [
            "submit",
            "formaction",
            "autofocus",
            "iframe",
            "<svg",
            "onload",
        ] {
            assert!(!html.contains(unsafe_text));
        }
    }
}
