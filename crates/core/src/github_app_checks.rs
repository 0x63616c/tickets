//! Runtime-independent GitHub App signing and Tickets check output.
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use serde_json::{Value, json};

pub fn base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn der(tag: u8, bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    if bytes.len() < 128 {
        out.push(bytes.len() as u8);
    } else {
        let length = bytes.len().to_be_bytes();
        let length = &length[length.iter().position(|b| *b != 0).unwrap()..];
        out.push(0x80 | length.len() as u8);
        out.extend_from_slice(length);
    }
    out.extend_from_slice(bytes);
    out
}

pub fn pkcs1_to_pkcs8(bytes: &[u8]) -> Vec<u8> {
    // PrivateKeyInfo: version 0, rsaEncryption + NULL, privateKey OCTET STRING.
    let mut info = vec![
        0x02, 0x01, 0x00, 0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01,
        0x01, 0x05, 0x00,
    ];
    info.extend(der(0x04, bytes));
    der(0x30, &info)
}

/// Decode a single unencrypted RSA PEM. WebCrypto validates the DER key itself.
pub fn pem_pkcs8(pem: &str) -> Option<Vec<u8>> {
    let pem = pem.trim();
    for (label, rsa) in [("RSA PRIVATE KEY", true), ("PRIVATE KEY", false)] {
        if let Some(body) = pem
            .strip_prefix(&format!("-----BEGIN {label}-----"))
            .and_then(|p| p.strip_suffix(&format!("-----END {label}-----")))
        {
            let encoded: String = body.chars().filter(|c| !c.is_ascii_whitespace()).collect();
            let decoded = STANDARD.decode(encoded).ok()?;
            if decoded.first() != Some(&0x30) {
                return None;
            }
            return Some(if rsa {
                pkcs1_to_pkcs8(&decoded)
            } else {
                decoded
            });
        }
    }
    None
}

pub fn jwt_signing_input(app_id: i64, now: i64) -> String {
    let header = base64url(br#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = json!({"iat":now-60,"exp":now+540,"iss":app_id.to_string()});
    format!("{header}.{}", base64url(claims.to_string().as_bytes()))
}

fn markdown_text(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_control() {
                vec![' ']
            } else if "\\`*_{}[]<>()!#|".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

/// Existing tickets are resolved in one database query using the shared links.
pub fn check_output(
    head_sha: &str,
    links: &[crate::pull_request::Link],
    tickets: &[Value],
) -> Value {
    let mut output =
        json!({"name":"Tickets","head_sha":head_sha,"status":"completed","conclusion":"neutral"});
    let (title, summary) = if links.is_empty() {
        ("No ticket key".to_owned(),"Add `[KEY-N]` to the pull request title and `Closes KEY-N` to the body (or `Refs KEY-N` for a reference).".to_owned())
    } else if tickets.is_empty() {
        (
            "Unknown ticket key".to_owned(),
            format!(
                "No matching Tickets ticket exists for: {}.",
                links
                    .iter()
                    .map(|l| format!("{}-{}", l.prefix, l.number))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    } else {
        output["conclusion"] = json!("success");
        let mut keys = vec![];
        let mut lines = vec![];
        for link in links {
            let key = format!("{}-{}", link.prefix, link.number);
            if let Some(ticket) = tickets.iter().find(|t| t["key"].as_str() == Some(&key)) {
                let url = format!("https://tickets.worldwidewebb.co/t/{key}");
                if keys.is_empty() {
                    output["details_url"] = json!(url);
                }
                keys.push(key.clone());
                lines.push(format!(
                    "- {} [{key}]({url}) — {} ({})",
                    if link.closes { "Closes" } else { "Refs" },
                    markdown_text(ticket["title"].as_str().unwrap_or_default()),
                    markdown_text(
                        &ticket["status"]
                            .as_str()
                            .unwrap_or_default()
                            .replace('_', " ")
                    )
                ));
            } else {
                lines.push(format!("- Unknown ticket: {key}"));
            }
        }
        // GitHub limits output titles to 255 characters.
        (
            format!("Linked {}", keys.join(", "))
                .chars()
                .take(255)
                .collect(),
            lines.join("\n"),
        )
    };
    output["output"] =
        json!({"title":title,"summary":summary.chars().take(65000).collect::<String>()});
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signing_input_and_base64() {
        assert_eq!(base64url(&[251, 255]), "-_8");
        let input = jwt_signing_input(42, 1000);
        let parts: Vec<_> = input.split('.').collect();
        assert_eq!(
            serde_json::from_slice::<Value>(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap(),
            json!({"alg":"RS256","typ":"JWT"})
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap(),
            json!({"iat":940,"exp":1540,"iss":"42"})
        );
    }
    #[test]
    fn der_lengths() {
        assert_eq!(der(4, &[0; 127])[..2], [4, 127]);
        assert_eq!(der(4, &[0; 128])[..3], [4, 0x81, 128]);
        assert_eq!(der(4, &[0; 256])[..4], [4, 0x82, 1, 0]);
        assert!(pem_pkcs8("bad").is_none());
        assert!(
            pem_pkcs8("-----BEGIN PRIVATE KEY-----\nAA==\n-----END PRIVATE KEY-----").is_none()
        );
    }
    #[test]
    fn throwaway_fixture_conversion() {
        let rsa = include_str!("../tests/fixtures/github-app-test-rsa.pem");
        let pkcs8 = include_str!("../tests/fixtures/github-app-test-pkcs8.pem");
        assert_eq!(pem_pkcs8(rsa), pem_pkcs8(pkcs8));
        assert!(pem_pkcs8(rsa).unwrap().len() > 1000);
    }
    #[test]
    fn check_variants_and_markdown() {
        let links = crate::pull_request::links("[TIK-12] [TIK-13]", "Refs TIK-13");
        assert_eq!(
            check_output("sha", &[], &[])["output"]["title"],
            "No ticket key"
        );
        assert_eq!(check_output("sha", &links, &[])["conclusion"], "neutral");
        let output = check_output(
            "sha",
            &links,
            &[
                json!({"key":"TIK-12","title":"A [title] <x>","status":"in_review"}),
                json!({"key":"TIK-13","title":"Second","status":"todo"}),
            ],
        );
        assert_eq!(output["conclusion"], "success");
        assert_eq!(
            output["details_url"],
            "https://tickets.worldwidewebb.co/t/TIK-12"
        );
        let summary = output["output"]["summary"].as_str().unwrap();
        assert!(summary.contains("Closes [TIK-12]"));
        assert!(summary.contains("Refs [TIK-13]"));
        assert!(summary.contains(r"A \[title\] \<x\>"));
        assert!(summary.contains("in review"));
    }
}
