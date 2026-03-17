use subtle::ConstantTimeEq;

/// Constant-time check whether `token` matches any entry in `configured_tokens`.
///
/// Iterates over all configured tokens regardless of where a match occurs,
/// preventing timing side-channels from leaking token ordering or count.
#[must_use]
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

    #[test]
    fn matches_empty_token_against_list() {
        let tokens = vec!["nonempty".to_string()];
        assert!(!constant_time_token_matches("", &tokens));
    }

    #[test]
    fn matches_empty_token_in_list() {
        let tokens = vec![String::new()];
        assert!(constant_time_token_matches("", &tokens));
    }

    #[test]
    fn matches_single_element_list_hit() {
        let tokens = vec!["only".to_string()];
        assert!(constant_time_token_matches("only", &tokens));
    }

    #[test]
    fn matches_single_element_list_miss() {
        let tokens = vec!["only".to_string()];
        assert!(!constant_time_token_matches("other", &tokens));
    }

    #[test]
    fn eq_identical_long_tokens() {
        let long = "a".repeat(10_000);
        assert!(constant_time_token_eq(&long, &long));
    }

    #[test]
    fn eq_different_long_tokens_same_length() {
        let a = "a".repeat(10_000);
        let b = "b".repeat(10_000);
        assert!(!constant_time_token_eq(&a, &b));
    }

    #[test]
    fn eq_unicode_tokens() {
        assert!(constant_time_token_eq("café", "café"));
    }

    #[test]
    fn eq_unicode_tokens_different() {
        assert!(!constant_time_token_eq("café", "cafe\u{0301}")); // NFC vs NFD
    }

    #[test]
    fn eq_tokens_with_null_byte() {
        assert!(!constant_time_token_eq("abc\x00def", "abcdef"));
    }

    #[test]
    fn eq_single_byte_tokens() {
        assert!(constant_time_token_eq("a", "a"));
        assert!(!constant_time_token_eq("a", "b"));
    }

    #[test]
    fn matches_unicode_token_in_list() {
        let tokens = vec!["α".to_string(), "β".to_string()];
        assert!(constant_time_token_matches("β", &tokens));
    }

    #[test]
    fn matches_token_with_special_chars() {
        let tokens = vec![
            "Bearer abc123!".to_string(),
            "key=value;other=thing".to_string(),
        ];
        assert!(constant_time_token_matches("Bearer abc123!", &tokens));
    }

    #[test]
    fn matches_no_false_positive_on_prefix() {
        let tokens = vec!["secret".to_string()];
        assert!(!constant_time_token_matches("secret-longer", &tokens));
    }

    #[test]
    fn matches_no_false_positive_on_suffix() {
        let tokens = vec!["secret".to_string()];
        assert!(!constant_time_token_matches("my-secret", &tokens));
    }

    #[rstest::rstest]
    #[case("", "", true)]
    #[case("a", "a", true)]
    #[case("a", "b", false)]
    #[case("hello", "world", false)]
    #[case("same", "same", true)]
    fn parameterized_token_eq(#[case] a: &str, #[case] b: &str, #[case] expected: bool) {
        assert_eq!(constant_time_token_eq(a, b), expected);
    }
}
