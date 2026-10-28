//! Host-testable transport errors and sign-in recovery policy.
#[derive(Debug)]
pub(crate) enum ApiError {
    Network,
    Http { status: u16, message: String },
    InvalidResponse,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network => f.write_str("Connection failed"),
            Self::Http { message, .. } => f.write_str(message),
            Self::InvalidResponse => f.write_str("Invalid response"),
        }
    }
}

impl ApiError {
    pub(crate) fn login_message(&self) -> String {
        match self {
            Self::Http { status: 401, .. } => "Incorrect username or password.".into(),
            Self::Http { status: 429, .. } => {
                "Too many sign-in attempts. Wait a few minutes and try again.".into()
            }
            Self::Network => "Connection failed. Check your connection and try again.".into(),
            _ => "Unable to sign in right now. Please try again.".into(),
        }
    }
}

pub(crate) fn clear_password(result: Result<(), &ApiError>) -> bool {
    matches!(result, Ok(()) | Err(ApiError::Http { status: 401, .. }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_and_rejected_credentials_clear_password() {
        assert!(clear_password(Ok(())));
        let error = ApiError::Http {
            status: 401,
            message: "Unauthorized".into(),
        };
        assert!(clear_password(Err(&error)));
        assert_eq!(error.login_message(), "Incorrect username or password.");
    }

    #[test]
    fn retryable_errors_keep_password() {
        for error in [
            ApiError::Http {
                status: 429,
                message: "Rate limited".into(),
            },
            ApiError::Http {
                status: 500,
                message: "Server error".into(),
            },
            ApiError::Network,
            ApiError::InvalidResponse,
        ] {
            assert!(!clear_password(Err(&error)));
            assert!(error.login_message().contains("try again"));
            assert!(!error.to_string().is_empty());
        }
    }
}
