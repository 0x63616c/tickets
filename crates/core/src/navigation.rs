//! Shared redirect policy. Inputs are decoded exactly once by the URL query parser.
pub fn safe_next(value: &str) -> &str {
    if value.len() > 2048
        || !value.starts_with('/')
        || value.starts_with("//")
        || value.contains('\\')
        || value.chars().any(char::is_control)
        // Reject encoded separators/control bytes too, including nested encodings.
        || value.as_bytes().windows(3).any(|w| {
            w[0] == b'%' && u8::from_str_radix(std::str::from_utf8(&w[1..]).unwrap_or(""), 16)
                .is_ok_and(|b| b < 32 || b == 127 || b == b'\\' || b == b'/' || b == b'%')
        })
    {
        "/"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_external_and_ambiguous_destinations() {
        for value in [
            "",
            "//evil.com",
            "/\\evil.com",
            "https://evil.com",
            "javascript:alert(1)",
            " /list",
            "/a\\b",
            "/\nfoo",
            "/%2fevil.com",
            "/%5Cevil",
            "/%0dfoo",
            "/%252fevil",
        ] {
            assert_eq!(safe_next(value), "/", "{value:?}");
        }
        assert_eq!(safe_next(&format!("/{}", "a".repeat(2048))), "/");
    }
    #[test]
    fn preserves_relative_path_and_query() {
        for value in [
            "/",
            "/ticket?id=1",
            "/list?q=hello%20world",
            "/unknown?x=1&y=2",
        ] {
            assert_eq!(safe_next(value), value);
        }
    }
}
