//! Shared redirect policy. Inputs are decoded exactly once by the URL query parser.
pub fn safe_next(value: &str) -> &str {
    if value.len() > 2048
        || !value.starts_with('/')
        || value.starts_with("//")
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || unsafe_encoded_path(value.split(['?', '#']).next().unwrap_or(""))
    {
        "/"
    } else {
        value
    }
}

// Decode path escapes repeatedly so nested percent encodings cannot hide separators.
fn unsafe_encoded_path(path: &str) -> bool {
    let mut bytes = path.as_bytes().to_vec();
    loop {
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut changed = false;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%'
                && i + 2 < bytes.len()
                && let Ok(byte) =
                    u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                if byte < 32 || byte == 127 || byte == b'/' || byte == b'\\' {
                    return true;
                }
                decoded.push(byte);
                changed = true;
                i += 3;
            } else {
                decoded.push(bytes[i]);
                i += 1;
            }
        }
        if !changed {
            return false;
        }
        bytes = decoded;
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
            "/%2f/evil.com",
            "/%5Cevil",
            "/%0dfoo",
            "/%252fevil",
            "/%25250afoo",
            "/%7ffoo",
            "/list?q=raw\\backslash",
            "/list?q=raw\ncontrol",
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
            "/t/QUA-87",
            "/t/qua-87",
            "/list?q=hello%20world",
            "/list?q=a%2Fb",
            "/list?q=100%25",
            "/list?q=%5c%00%252f",
            "/list#%2f%25",
            "/percent%25literal",
            "/percent%2525literal",
            "/unknown?x=1&y=2",
        ] {
            assert_eq!(safe_next(value), value);
        }
    }
}
