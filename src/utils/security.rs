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

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(128))]

            /// Token matching is commutative: result is the same regardless
            /// of list order, preventing position-based timing leaks.
            #[test]
            fn token_matching_is_commutative(
                token in "[a-zA-Z0-9]{1,30}",
                t1 in "[a-zA-Z0-9]{1,30}",
                t2 in "[a-zA-Z0-9]{1,30}",
            ) {
                let list1 = vec![t1.clone(), t2.clone()];
                let list2 = vec![t2, t1];
                prop_assert_eq!(
                    constant_time_token_matches(&token, &list1),
                    constant_time_token_matches(&token, &list2),
                );
            }

            /// An empty list never matches any token.
            #[test]
            fn empty_list_always_false(token in "\\PC{0,100}") {
                prop_assert!(!constant_time_token_matches(&token, &[]));
            }

            /// A token always matches itself in a single-element list.
            #[test]
            fn token_matches_itself(token in "[a-zA-Z0-9]{1,30}") {
                let list = vec![token.clone()];
                prop_assert!(constant_time_token_matches(&token, &list));
            }

            /// Any byte sequence as token never panics against an empty list.
            #[test]
            fn any_token_never_panics(token: Vec<u8>) {
                let token_str = String::from_utf8_lossy(&token);
                let result = constant_time_token_matches(&token_str, &[]);
                prop_assert!(!result, "Empty list should never match");
            }
        }
    }
}
