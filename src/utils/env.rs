/// Expand environment variable references in a string.
///
/// Supports `${VAR_NAME}`, `${VAR_NAME:-default}`, and embedded references,
/// e.g., `"Bearer ${AUTH_KEY}"` or `"${HOST:-localhost}:${PORT}"`.
#[must_use]
pub fn expand_env_var(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];

        if let Some(end) = rest.find('}') {
            let inner = &rest[..end];
            rest = &rest[end + 1..];

            let expanded = if let Some((var_name, default)) = inner.split_once(":-") {
                std::env::var(var_name).unwrap_or_else(|_| default.to_string())
            } else {
                std::env::var(inner).unwrap_or_default()
            };
            result.push_str(&expanded);
        } else {
            // No closing brace, treat as literal
            result.push_str("${");
        }
    }

    result.push_str(rest);
    result
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static ENV_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn expands_set_variable() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        // ALLOW: test-only env mutation
        #[allow(clippy::expect_used)]
        unsafe {
            std::env::set_var("TEST_UTILS_API_KEY", "secret-value");
        }
        let expanded = expand_env_var("${TEST_UTILS_API_KEY}");
        assert_eq!(expanded, "secret-value");
        unsafe {
            std::env::remove_var("TEST_UTILS_API_KEY");
        }
    }

    #[test]
    fn expands_with_default_when_unset() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        let expanded = expand_env_var("${NONEXISTENT_VAR_UTILS:-default-value}");
        assert_eq!(expanded, "default-value");
    }

    #[test]
    fn literal_value_unchanged() {
        let literal = expand_env_var("literal-value");
        assert_eq!(literal, "literal-value");
    }

    #[test]
    fn embedded_references() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        // ALLOW: test-only env mutation
        #[allow(clippy::expect_used)]
        unsafe {
            std::env::set_var("TEST_UTILS_HOST", "example.com");
        }
        let expanded = expand_env_var("https://${TEST_UTILS_HOST}/api");
        assert_eq!(expanded, "https://example.com/api");
        unsafe {
            std::env::remove_var("TEST_UTILS_HOST");
        }
    }

    #[test]
    fn unclosed_brace_treated_as_literal() {
        let expanded = expand_env_var("${UNCLOSED");
        assert_eq!(expanded, "${UNCLOSED");
    }

    #[test]
    fn unset_var_expands_to_empty() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        let expanded = expand_env_var("${SURELY_NONEXISTENT_VAR_XYZ}");
        assert_eq!(expanded, "");
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(128))]

            /// Strings without `${` pass through unchanged.
            #[test]
            fn no_expansion_marker_returns_input(input in "[^$]*") {
                let result = expand_env_var(&input);
                prop_assert_eq!(result, input);
            }

            /// Any string can be processed without panicking.
            #[test]
            fn malformed_patterns_never_panic(input in ".*") {
                let _ = expand_env_var(&input);
            }

            /// Unclosed brace patterns are treated as literals.
            #[test]
            fn unclosed_brace_is_literal(prefix in "[^$]*", var in "[^}]*") {
                let input = format!("{prefix}${{{var}");
                let result = expand_env_var(&input);
                prop_assert!(
                    result.contains("${") || result == input,
                    "Unclosed brace should preserve literal: input={input}, result={result}"
                );
            }

            /// Nonexistent env vars expand to empty string.
            #[test]
            fn nonexistent_var_expands_to_empty(
                prefix in "[a-z]{0,10}",
                suffix in "[a-z]{0,10}",
            ) {
                let input = format!("{prefix}${{SURELY_NONEXISTENT_PROPTST_XYZ}}{suffix}");
                let result = expand_env_var(&input);
                prop_assert_eq!(result, format!("{prefix}{suffix}"));
            }
        }
    }
}
