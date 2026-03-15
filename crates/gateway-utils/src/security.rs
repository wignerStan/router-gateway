use subtle::ConstantTimeEq;

/// Constant-time check whether `token` matches any entry in `configured_tokens`.
///
/// Iterates over all configured tokens regardless of where a match occurs,
/// preventing timing side-channels from leaking token ordering or count.
pub fn constant_time_token_matches(token: &str, configured_tokens: &[String]) -> bool {
    let token_bytes = token.as_bytes();
    let mut result: u8 = 0;
    for configured in configured_tokens {
        let eq = configured.as_bytes().ct_eq(token_bytes).unwrap_u8();
        result |= eq;
    }
    result != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use subtle::ConstantTimeEq;

    /// Constant-time comparison of two token strings to prevent timing attacks.
    /// Returns `true` if tokens are byte-equal, always comparing the full length
    /// of both strings regardless of where they differ.
    fn constant_time_token_eq(a: &str, b: &str) -> bool {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();
        a_bytes.ct_eq(b_bytes).into()
    }

    #[test]
    fn same_tokens_are_equal() {
        assert!(constant_time_token_eq(
            "secret-token-123",
            "secret-token-123"
        ));
    }

    #[test]
    fn different_tokens_same_length_are_not_equal() {
        assert!(!constant_time_token_eq(
            "secret-token-123",
            "secret-token-124"
        ));
    }

    #[test]
    fn different_lengths_are_not_equal() {
        assert!(!constant_time_token_eq("short", "much-longer-token"));
    }

    #[test]
    fn empty_tokens_are_equal() {
        assert!(constant_time_token_eq("", ""));
    }

    #[test]
    fn one_empty_is_not_equal() {
        assert!(!constant_time_token_eq("nonempty", ""));
    }

    #[test]
    fn matches_hit() {
        let tokens = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        assert!(constant_time_token_matches("beta", &tokens));
    }

    #[test]
    fn matches_miss() {
        let tokens = vec!["alpha".to_string(), "beta".to_string()];
        assert!(!constant_time_token_matches("delta", &tokens));
    }

    #[test]
    fn matches_empty_list() {
        assert!(!constant_time_token_matches("anything", &[]));
    }

    #[test]
    fn matches_first() {
        let tokens = vec!["first".to_string(), "second".to_string()];
        assert!(constant_time_token_matches("first", &tokens));
    }

    #[test]
    fn matches_last() {
        let tokens = vec!["first".to_string(), "last".to_string()];
        assert!(constant_time_token_matches("last", &tokens));
    }
}
