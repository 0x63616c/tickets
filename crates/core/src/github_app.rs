//! Manifest registration rules and presentation. Credential types intentionally lack Debug.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const IV_BYTES: usize = 12;
const TAG_BYTES: usize = 16;

#[derive(Deserialize)]
pub struct AppMetadata {
    pub app_id: i64,
    pub slug: String,
    pub name: String,
    pub owner_login: String,
    pub client_id: String,
    pub html_url: String,
}

#[derive(Deserialize, Serialize)]
pub struct AppSecrets {
    pub pem: String,
    pub webhook_secret: String,
    pub client_secret: String,
}

impl AppSecrets {
    fn valid(&self) -> bool {
        [&self.pem, &self.webhook_secret, &self.client_secret]
            .iter()
            .all(|value| !value.trim().is_empty())
    }
}

pub struct AppCredentials {
    pub app: AppMetadata,
    pub secrets: AppSecrets,
}

/// Hash to fixed-size digests before constant-time comparison, including unequal lengths.
pub fn state_matches(expected: &str, actual: &str) -> bool {
    crate::auth::constant_eq(
        &Sha256::digest(expected.as_bytes()),
        &Sha256::digest(actual.as_bytes()),
    )
}

pub fn valid_code(code: &str) -> bool {
    (1..=128).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

pub fn store_key(encoded: &str) -> Option<[u8; 32]> {
    STANDARD.decode(encoded).ok()?.try_into().ok()
}

#[derive(Debug, PartialEq, Eq)]
pub enum CallbackError {
    Unavailable,
    BadState,
    BadCode,
    AlreadyRegistered,
    ExchangeFailed(Option<u16>),
    Internal,
    MethodNotAllowed,
}

impl CallbackError {
    pub fn status(&self) -> u16 {
        match self {
            Self::Unavailable => 503,
            Self::BadState => 403,
            Self::BadCode => 400,
            Self::AlreadyRegistered => 409,
            Self::ExchangeFailed(_) => 502,
            Self::Internal => 500,
            Self::MethodNotAllowed => 405,
        }
    }
}

/// No storage or network access until configuration and callback inputs are valid.
pub fn validate_callback(
    setup_state: Option<&str>,
    encoded_key: Option<&str>,
    state: &str,
    code: &str,
) -> Result<[u8; 32], CallbackError> {
    let expected = setup_state
        .filter(|s| !s.is_empty())
        .ok_or(CallbackError::Unavailable)?;
    let key = encoded_key
        .and_then(store_key)
        .ok_or(CallbackError::Unavailable)?;
    if !state_matches(expected, state) {
        return Err(CallbackError::BadState);
    }
    if !valid_code(code) {
        return Err(CallbackError::BadCode);
    }
    Ok(key)
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty() && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn valid_app_url(app: &AppMetadata) -> bool {
    valid_slug(&app.slug) && app.html_url == format!("https://github.com/apps/{}", app.slug)
}

/// Ignore additional GitHub fields, but reject incomplete credentials and unsafe links.
/// Parse failures never carry any part of GitHub's response into an error.
pub fn parse_conversion(bytes: &[u8]) -> Option<AppCredentials> {
    #[derive(Deserialize)]
    struct Owner {
        login: String,
    }
    #[derive(Deserialize)]
    struct Conversion {
        id: i64,
        slug: String,
        name: String,
        owner: Owner,
        client_id: String,
        html_url: String,
        #[serde(flatten)]
        secrets: AppSecrets,
    }
    let response: Conversion = serde_json::from_slice(bytes).ok()?;
    let app = AppMetadata {
        app_id: response.id,
        slug: response.slug,
        name: response.name,
        owner_login: response.owner.login,
        client_id: response.client_id,
        html_url: response.html_url,
    };
    (app.app_id > 0
        && valid_app_url(&app)
        && [&app.name, &app.owner_login, &app.client_id]
            .iter()
            .all(|value| !value.trim().is_empty())
        && response.secrets.valid())
    .then_some(AppCredentials {
        app,
        secrets: response.secrets,
    })
}

pub fn parse_secrets(bytes: &[u8]) -> Option<AppSecrets> {
    let secrets: AppSecrets = serde_json::from_slice(bytes).ok()?;
    secrets.valid().then_some(secrets)
}

/// WebCrypto appends the 128-bit authentication tag to its ciphertext.
pub fn pack_ciphertext(iv: &[u8; IV_BYTES], ciphertext: &[u8]) -> Option<String> {
    if ciphertext.len() < TAG_BYTES {
        return None;
    }
    let mut packed = iv.to_vec();
    packed.extend_from_slice(ciphertext);
    Some(STANDARD.encode(packed))
}

pub fn unpack_ciphertext(encoded: &str) -> Option<([u8; IV_BYTES], Vec<u8>)> {
    let packed = STANDARD.decode(encoded).ok()?;
    if packed.len() < IV_BYTES + TAG_BYTES {
        return None;
    }
    Some((
        packed[..IV_BYTES].try_into().ok()?,
        packed[IV_BYTES..].to_vec(),
    ))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn page_style() -> String {
    // Embed the authoritative dark tokens, without font-face rules or external assets.
    let tokens = include_str!("../../web/style/tokens.css")
        .split_once(":root {")
        .expect("design tokens must have a root block")
        .1
        .split_once('}')
        .expect("design token root block must close")
        .0;
    format!(
        ":root {{{tokens}}}\n{}",
        include_str!("../../web/style/github-app.css")
    )
}

pub fn content_security_policy() -> String {
    let hash = STANDARD.encode(Sha256::digest(page_style().as_bytes()));
    format!(
        "default-src 'none'; style-src 'sha256-{hash}'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    )
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"referrer\" content=\"no-referrer\"><title>{} · Tickets</title><style>{}</style></head><body><main><h1>{}</h1>{body}</main></body></html>",
        escape_html(title),
        page_style(),
        escape_html(title),
    )
}

/// Only public metadata reaches the renderer; secrets are never accepted here.
pub fn registration_html(app: &AppMetadata) -> String {
    let links = if valid_app_url(app) {
        format!(
            "<p><a class=\"install\" href=\"https://github.com/apps/{}/installations/new\" rel=\"noreferrer\">Install the app</a></p><p><a href=\"{}\" rel=\"noreferrer\">View app on GitHub</a></p>",
            escape_html(&app.slug),
            escape_html(&app.html_url),
        )
    } else {
        String::new()
    };
    page(
        "GitHub App registered",
        &format!(
            "<p>The app &quot;{}&quot; was registered. Install the app to connect your repositories.</p>{links}<p><a href=\"/settings\">Back to Settings</a></p>",
            escape_html(&app.name),
        ),
    )
}

// Keep script bytes fixed so the CSP can authorize exactly this script by hash.
const AUTO_SUBMIT: &str =
    "window.addEventListener('load',()=>document.getElementById('f').requestSubmit());";

pub fn register_content_security_policy() -> String {
    let style_hash = STANDARD.encode(Sha256::digest(page_style().as_bytes()));
    let script_hash = STANDARD.encode(Sha256::digest(AUTO_SUBMIT.as_bytes()));
    format!(
        "default-src 'none'; style-src 'sha256-{style_hash}'; script-src 'sha256-{script_hash}'; base-uri 'none'; form-action https://github.com; frame-ancestors 'none'"
    )
}

pub fn registration_unavailable_html() -> String {
    page(
        "Registration unavailable",
        "<p>Check the app setup configuration before registering.</p>",
    )
}

/// The caller supplies an HTTP(S) request origin and the server-side setup state.
/// Query input only selects a fixed alternate name; it is never reflected.
pub fn register_html(
    origin: &str,
    setup_state: &str,
    name: Option<&str>,
    registered: Option<&AppMetadata>,
) -> String {
    if let Some(app) = registered {
        let link = if valid_app_url(app) {
            format!(
                "<p><a href=\"{}\" rel=\"noreferrer\">View app on GitHub</a></p>",
                escape_html(&app.html_url),
            )
        } else {
            String::new()
        };
        return page(
            "GitHub App already registered",
            &format!("<p>This workspace already has a registered GitHub App.</p>{link}"),
        );
    }
    let manifest = serde_json::json!({
        "name": if name == Some("alt") { "Tickets WWW" } else { "Tickets by World Wide Webb" },
        "url": origin,
        "description": "Links pull requests to Tickets, moves tickets by PR status and posts a Tickets check on every PR.",
        "hook_attributes": {"url": format!("{origin}/api/v1/github/app/webhook"), "active": true},
        "redirect_url": format!("{origin}/api/v1/github/app/manifest-callback"),
        "setup_url": format!("{origin}/settings"),
        "setup_on_update": false,
        "public": false,
        "default_permissions": {"pull_requests": "read", "issues": "read", "checks": "write", "statuses": "write", "metadata": "read", "contents": "read"},
        "default_events": ["pull_request", "check_suite", "check_run", "push"]
    });
    let state: String = setup_state
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    page(
        "Create GitHub App",
        &format!(
            "<p>Redirecting to GitHub…</p><form id=\"f\" method=\"post\" action=\"https://github.com/settings/apps/new?state={state}\"><input type=\"hidden\" name=\"manifest\" value=\"{}\"><button class=\"install\" type=\"submit\">Continue to GitHub</button></form><script>{AUTO_SUBMIT}</script>",
            escape_html(&manifest.to_string()),
        ),
    )
}

pub fn error_html(error: &CallbackError) -> String {
    let (title, message) = match error {
        CallbackError::Unavailable => (
            "App setup unavailable",
            "Check the app setup configuration before registering.".into(),
        ),
        CallbackError::BadState => (
            "App setup denied",
            "Start registration again with the configured setup state.".into(),
        ),
        CallbackError::BadCode => (
            "Invalid registration code",
            "Start registration again from GitHub.".into(),
        ),
        CallbackError::AlreadyRegistered => (
            "GitHub App already registered",
            "This workspace already has a registered GitHub App.".into(),
        ),
        CallbackError::ExchangeFailed(status) => (
            "Registration could not be completed",
            status.map_or_else(
                || "GitHub app conversion failed.".into(),
                |status| format!("GitHub app conversion failed (HTTP {status})."),
            ),
        ),
        CallbackError::Internal => (
            "Registration could not be saved",
            "Check the app setup configuration and storage before trying again.".into(),
        ),
        CallbackError::MethodNotAllowed => (
            "Method not allowed",
            "Use the registration link provided by GitHub.".into(),
        ),
    };
    page(title, &format!("<p>{}</p>", escape_html(&message)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn conversion() -> serde_json::Value {
        json!({
            "id": 42, "slug": "fixture-app", "name": "Fixture App",
            "owner": {"login": "fixture-owner"}, "client_id": "fixture-client-id",
            "html_url": "https://github.com/apps/fixture-app",
            "pem": "fixture-only-pem", "webhook_secret": "fixture-only-webhook",
            "client_secret": "fixture-only-client", "extra_github_field": true
        })
    }

    #[test]
    fn state_comparison_handles_lengths_and_unicode() {
        assert!(state_matches("fixture-state", "fixture-state"));
        for other in [
            "",
            "fixture-stat",
            "fixture-state-extra",
            "Fixture-state",
            "é",
        ] {
            assert!(!state_matches("fixture-state", other));
        }
        assert!(!state_matches("é", "e\u{301}"));
    }

    #[test]
    fn code_is_a_bounded_ascii_path_segment() {
        for code in ["a", "AZaz09_-", &"x".repeat(128)] {
            assert!(valid_code(code));
        }
        for code in [
            "",
            "a/b",
            "..",
            "a?b",
            "%2f",
            "a b",
            "a\n",
            "é",
            &"x".repeat(129),
        ] {
            assert!(!valid_code(code));
        }
    }

    #[test]
    fn configuration_and_inputs_fail_closed_in_order() {
        let key = STANDARD.encode([7; 32]);
        assert_eq!(
            validate_callback(Some("s"), Some(&key), "s", "c"),
            Ok([7; 32])
        );
        for (state, key) in [
            (None, Some(key.as_str())),
            (Some(""), Some(&key)),
            (Some("s"), None),
            (Some("s"), Some("")),
            (Some("s"), Some("invalid")),
        ] {
            assert_eq!(
                validate_callback(state, key, "wrong", ""),
                Err(CallbackError::Unavailable)
            );
        }
        assert_eq!(
            validate_callback(Some("s"), Some(&key), "wrong", ""),
            Err(CallbackError::BadState)
        );
        assert_eq!(
            validate_callback(Some("s"), Some(&key), "s", ""),
            Err(CallbackError::BadCode)
        );
        for length in [0, 16, 31, 33, 64] {
            assert!(store_key(&STANDARD.encode(vec![7; length])).is_none());
        }
    }

    #[test]
    fn conversion_requires_metadata_and_complete_secrets() {
        let response = conversion();
        let parsed = parse_conversion(&serde_json::to_vec(&response).unwrap()).unwrap();
        assert_eq!(parsed.app.app_id, 42);
        assert_eq!(parsed.app.owner_login, "fixture-owner");
        assert_eq!(parsed.secrets.pem, "fixture-only-pem");
        let secrets = serde_json::to_value(&parsed.secrets).unwrap();
        assert_eq!(secrets.as_object().unwrap().len(), 3);
        assert!(parse_secrets(&serde_json::to_vec(&secrets).unwrap()).is_some());
        for field in [
            "id",
            "slug",
            "name",
            "owner",
            "client_id",
            "html_url",
            "pem",
            "webhook_secret",
            "client_secret",
        ] {
            let mut missing = response.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                parse_conversion(&serde_json::to_vec(&missing).unwrap()).is_none(),
                "{field}"
            );
        }
        for (field, value) in [
            ("id", json!(0)),
            ("id", json!(1.5)),
            ("owner", json!({})),
            ("name", json!("")),
            ("pem", json!(null)),
            ("webhook_secret", json!("")),
            ("client_secret", json!("  ")),
            ("slug", json!("../settings")),
            ("html_url", json!("javascript:alert(1)")),
            (
                "html_url",
                json!("https://github.com.evil/apps/fixture-app"),
            ),
        ] {
            let mut invalid = response.clone();
            invalid[field] = value;
            assert!(
                parse_conversion(&serde_json::to_vec(&invalid).unwrap()).is_none(),
                "{field}"
            );
        }
        assert!(parse_conversion(b"not json").is_none());
        assert!(parse_secrets(b"{}").is_none());
    }

    #[test]
    fn ciphertext_format_is_iv_then_ciphertext_and_tag_in_standard_base64() {
        let iv = [9; IV_BYTES];
        let ciphertext = [127; 35];
        let packed = pack_ciphertext(&iv, &ciphertext).unwrap();
        let bytes = STANDARD.decode(&packed).unwrap();
        assert_eq!(&bytes[..IV_BYTES], &iv);
        assert_eq!(&bytes[IV_BYTES..], &ciphertext);
        assert_eq!(unpack_ciphertext(&packed), Some((iv, ciphertext.to_vec())));
        assert!(pack_ciphertext(&iv, &[0; TAG_BYTES - 1]).is_none());
        for len in 0..IV_BYTES + TAG_BYTES {
            assert!(unpack_ciphertext(&STANDARD.encode(vec![0; len])).is_none());
        }
        for value in ["", "not base64!", "===="] {
            assert!(unpack_ciphertext(value).is_none());
        }
    }

    #[test]
    fn html_escapes_metadata_omits_credentials_and_has_no_external_assets() {
        let mut response = conversion();
        response["name"] = json!("<script>\"A&B's\"</script>");
        let mut parsed = parse_conversion(&serde_json::to_vec(&response).unwrap()).unwrap();
        let html = registration_html(&parsed.app);
        assert!(html.contains("&lt;script&gt;&quot;A&amp;B&#39;s&quot;&lt;/script&gt;"));
        assert!(html.contains("https://github.com/apps/fixture-app/installations/new"));
        assert!(html.contains("href=\"https://github.com/apps/fixture-app\""));
        assert!(html.contains("href=\"/settings\">Back to Settings"));
        assert!(html.contains("Install the app"));
        for forbidden in [
            "<script",
            "<link",
            "<img",
            "@font-face",
            "fixture-only",
            "fixture-client-id",
        ] {
            assert!(!html.contains(forbidden), "{forbidden}");
        }
        parsed.app.html_url = "javascript:alert(1)\" onmouseover=\"alert(1)".into();
        assert!(!registration_html(&parsed.app).contains("https://github.com"));
        assert!(registration_html(&parsed.app).contains("href=\"/settings\""));
        let style = html
            .split_once("<style>")
            .unwrap()
            .1
            .split_once("</style>")
            .unwrap()
            .0;
        let hash = STANDARD.encode(Sha256::digest(style.as_bytes()));
        let csp = content_security_policy();
        assert!(csp.contains(&format!("style-src 'sha256-{hash}'")));
        assert!(csp.contains("default-src 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(!csp.contains("unsafe-inline"));
        assert!(style.contains("--bg: #000000"));
    }

    fn manifest_from_html(html: &str) -> serde_json::Value {
        let escaped = html
            .split_once("name=\"manifest\" value=\"")
            .unwrap()
            .1
            .split_once('"')
            .unwrap()
            .0;
        assert!(!escaped.contains(['<', '>', '\'']));
        assert!(escaped.contains("&quot;"));
        let decoded = escaped
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&");
        serde_json::from_str(&decoded).unwrap()
    }

    #[test]
    fn register_form_encodes_state_and_round_trips_exact_manifest() {
        let origin = "https://tickets.worldwidewebb.co";
        let html = register_html(origin, "a&b=+ /?\"'é", None, None);
        assert!(html.contains("id=\"f\" method=\"post\" action=\"https://github.com/settings/apps/new?state=a%26b%3D%2B%20%2F%3F%22%27%C3%A9\""));
        assert!(html.contains(">Continue to GitHub</button>"));
        assert!(html.contains("Redirecting to GitHub…"));
        assert_eq!(
            manifest_from_html(&html),
            json!({
                "name": "Tickets by World Wide Webb",
                "url": origin,
                "description": "Links pull requests to Tickets, moves tickets by PR status and posts a Tickets check on every PR.",
                "hook_attributes": {"url": format!("{origin}/api/v1/github/app/webhook"), "active": true},
                "redirect_url": format!("{origin}/api/v1/github/app/manifest-callback"),
                "setup_url": format!("{origin}/settings"),
                "setup_on_update": false, "public": false,
                "default_permissions": {"pull_requests": "read", "issues": "read", "checks": "write", "statuses": "write", "metadata": "read", "contents": "read"},
                "default_events": ["pull_request", "check_suite", "check_run", "push"]
            })
        );
        let script = html
            .split_once("<script>")
            .unwrap()
            .1
            .split_once("</script>")
            .unwrap()
            .0;
        assert!(script.contains("requestSubmit()"));
        let hash = STANDARD.encode(Sha256::digest(script.as_bytes()));
        let csp = register_content_security_policy();
        assert!(csp.contains(&format!("script-src 'sha256-{hash}'")));
        assert!(csp.contains("form-action https://github.com;"));
        assert!(csp.contains("default-src 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(!csp.contains("unsafe-inline"));
    }

    #[test]
    fn register_name_is_allowlisted_and_manifest_is_attribute_escaped() {
        for (name, expected) in [
            (Some("alt"), "Tickets WWW"),
            (Some("<script>evil</script>"), "Tickets by World Wide Webb"),
            (None, "Tickets by World Wide Webb"),
        ] {
            let html = register_html("http://localhost:8787", "fixture-state", name, None);
            let manifest = manifest_from_html(&html);
            assert_eq!(manifest["name"], expected);
            assert_eq!(manifest["url"], "http://localhost:8787");
            assert_eq!(manifest["setup_url"], "http://localhost:8787/settings");
            assert!(!html.contains("evil"));
        }
        // Exercise every attribute escape even though real request origins are normalized.
        let origin = "https://fixture.invalid/<>&\"'";
        let html = register_html(origin, "s", None, None);
        assert_eq!(manifest_from_html(&html)["url"], origin);
    }

    #[test]
    fn existing_registration_has_app_link_without_form_script_or_state() {
        let credentials = parse_conversion(&serde_json::to_vec(&conversion()).unwrap()).unwrap();
        let html = register_html(
            "https://fixture.invalid",
            "fixture-state",
            None,
            Some(&credentials.app),
        );
        assert!(html.contains("already registered"));
        assert!(html.contains("href=\"https://github.com/apps/fixture-app\""));
        for forbidden in ["<form", "<script", "fixture-state", "name=\"manifest\""] {
            assert!(!html.contains(forbidden));
        }
    }

    #[test]
    fn error_pages_use_fixed_messages_and_only_upstream_status() {
        for (error, status, text) in [
            (CallbackError::Unavailable, 503, "unavailable"),
            (CallbackError::BadState, 403, "denied"),
            (CallbackError::BadCode, 400, "Invalid registration code"),
            (CallbackError::AlreadyRegistered, 409, "already registered"),
            (CallbackError::ExchangeFailed(Some(422)), 502, "HTTP 422"),
            (
                CallbackError::ExchangeFailed(None),
                502,
                "conversion failed",
            ),
            (CallbackError::Internal, 500, "could not be saved"),
            (CallbackError::MethodNotAllowed, 405, "Method not allowed"),
        ] {
            assert_eq!(error.status(), status);
            assert!(error_html(&error).contains(text));
        }
    }
}
