//! Detect accidental logging of secrets. Tests scan *messages*, never print
//! discovered values.

const SENSITIVE: &[&str] = &[
    "authorization",
    "bearer ",
    "jwt",
    "password",
    "token",
    "secret",
    "api_key",
    "api-key",
    "apikey",
];

/// True when a log line looks like it contains a credential keyword.
/// Used by regression tests; production code should not log these fields.
pub fn looks_sensitive(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    SENSITIVE.iter().any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_authorization_headers() {
        assert!(looks_sensitive("Authorization: Bearer abc"));
        assert!(looks_sensitive("password=hunter2"));
        assert!(!looks_sensitive(
            "authenticated request path=/api/v1/customers"
        ));
    }
}
