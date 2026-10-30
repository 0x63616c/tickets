//! Runtime-independent credential rules. Raw credentials never belong in storage.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const PASSWORD_ITERATIONS: u32 = 100_000;
pub const MAX_PASSWORD_ITERATIONS: u32 = 100_000;
pub fn hash_token(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect()
}
pub fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    bool::from(a.ct_eq(b))
}
pub fn valid_token(token: &str, api: bool) -> bool {
    let value = if api {
        match token.strip_prefix("tk_") {
            Some(v) => v,
            None => return false,
        }
    } else {
        token
    };
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
pub fn active(expires: &str, now: &str, revoked: Option<&str>) -> bool {
    revoked.is_none() && expires > now
}
/// Secret format: pbkdf2-sha256$<iterations>$<16-byte salt hex>$<32-byte digest hex>.
pub fn password_parts(value: &str) -> Option<(u32, Vec<u8>, Vec<u8>)> {
    let parts: Vec<_> = value.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2-sha256" {
        return None;
    }
    let count = parts[1];
    if count.is_empty() || count.starts_with('0') || !count.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let iterations = count.parse::<u32>().ok()?;
    if iterations > MAX_PASSWORD_ITERATIONS {
        return None;
    }
    let salt = unhex(parts[2])?;
    let hash = unhex(parts[3])?;
    (salt.len() == 16 && hash.len() == 32).then_some((iterations, salt, hash))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_password_iterations() {
        let salt = "ab".repeat(16);
        let digest = "cd".repeat(32);
        for count in [1, 99999, 100000] {
            assert_eq!(
                password_parts(&format!("pbkdf2-sha256${count}${salt}${digest}"))
                    .unwrap()
                    .0,
                count
            );
        }
        for count in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            " 1",
            "1 ",
            "1.0",
            "1e5",
            "100001",
            "600000",
            "4294967296",
            "١",
        ] {
            assert!(password_parts(&format!("pbkdf2-sha256${count}${salt}${digest}")).is_none());
        }
        for value in [
            format!("other$100000${salt}${digest}"),
            format!("pbkdf2-sha256$100000${}${digest}", "+a".repeat(16)),
            format!("pbkdf2-sha256$100000${salt}${digest}$extra"),
            format!("pbkdf2-sha256$100000$ab${digest}"),
            format!("pbkdf2-sha256$100000${salt}$cd"),
            format!("pbkdf2-sha256$100000${salt}${}", "zz".repeat(32)),
        ] {
            assert!(password_parts(&value).is_none());
        }
    }
    #[test]
    fn credentials() {
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(valid_token(&format!("tk_{}", "ab".repeat(32)), true));
        for t in ["tk_", "abc", "Bearer tk_x"] {
            assert!(!valid_token(t, true));
        }
        assert!(!valid_token(&"é".repeat(32), false));
        assert!(!constant_eq(b"abc", b"abd"));
        assert!(constant_eq(b"abc", b"abc"));
        assert!(active("2026-02-02", "2026-02-01", None));
        assert!(!active("2026-02-01", "2026-02-01", None));
        assert!(!active("2026-02-02", "2026-02-01", Some("revoked")));
        assert!(
            password_parts(&format!(
                "pbkdf2-sha256$100000${}${}",
                "ab".repeat(16),
                "cd".repeat(32)
            ))
            .is_some()
        );
        assert!(password_parts("pbkdf2-sha256$1$ab$cd").is_none());
    }
}

#[cfg(test)]
mod password_tests {
    use super::*;
    #[test]
    fn pbkdf2_known_vector_and_wrong_password() {
        let mut result = [0; 32];
        pbkdf2::pbkdf2_hmac::<sha2_legacy::Sha256>(b"password", b"salt", 100_000, &mut result);
        assert_eq!(
            hex(&result),
            "0394a2ede332c9a13eb82e9b24631604c31df978b4e2f0fbd2c549944f9d79a5"
        );
        let mut wrong = [0; 32];
        pbkdf2::pbkdf2_hmac::<sha2_legacy::Sha256>(b"wrong", b"salt", 100_000, &mut wrong);
        assert!(!constant_eq(&result, &wrong));
    }
}

/// Optional account label. Authentication continues to use the deployment username.
pub fn display_name(input: &serde_json::Value) -> Result<Option<String>, &'static str> {
    let obj = input.as_object().ok_or("Expected an account object")?;
    if obj.len() != 1 || !obj.contains_key("display_name") {
        return Err("Only display_name can be updated");
    }
    match &obj["display_name"] {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(name) => {
            let name = name.trim();
            if name.chars().count() > 80 || name.chars().any(char::is_control) {
                return Err(
                    "Display name must be at most 80 characters without control characters",
                );
            }
            Ok((!name.is_empty()).then(|| name.to_owned()))
        }
        _ => Err("Display name must be text or null"),
    }
}

#[cfg(test)]
mod account_tests {
    use super::display_name;
    use serde_json::json;
    #[test]
    fn validates_nullable_display_name() {
        assert_eq!(
            display_name(&json!({"display_name":"  Sample User  "})),
            Ok(Some("Sample User".into()))
        );
        for v in [json!({"display_name":null}), json!({"display_name":"  "})] {
            assert_eq!(display_name(&v), Ok(None));
        }
        for v in [
            json!({}),
            json!({"display_name":3}),
            json!({"display_name":"a", "username":"b"}),
            json!({"display_name":"x".repeat(81)}),
            json!({"display_name":"a\nb"}),
        ] {
            assert!(display_name(&v).is_err());
        }
    }
}
