//! Runtime-independent credential rules. Raw credentials never belong in storage.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const PASSWORD_ITERATIONS: u32 = 600_000;
pub fn hash_token(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) || !value.is_ascii() {
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
/// Secret format: pbkdf2-sha256$600000$<16-byte salt hex>$<32-byte digest hex>.
pub fn password_parts(value: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let parts: Vec<_> = value.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2-sha256" || parts[1] != "600000" {
        return None;
    }
    let salt = unhex(parts[2])?;
    let hash = unhex(parts[3])?;
    (salt.len() == 16 && hash.len() == 32).then_some((salt, hash))
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
                "pbkdf2-sha256$600000${}${}",
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
        pbkdf2::pbkdf2_hmac::<sha2_legacy::Sha256>(b"password", b"salt", 600_000, &mut result);
        assert_eq!(
            hex(&result),
            "669cfe52482116fda1aa2cbe409b2f56c8e4563752b7a28f6eaab614ee005178"
        );
        let mut wrong = [0; 32];
        pbkdf2::pbkdf2_hmac::<sha2_legacy::Sha256>(b"wrong", b"salt", 600_000, &mut wrong);
        assert!(!constant_eq(&result, &wrong));
    }
}
