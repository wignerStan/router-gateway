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
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static ENV_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn expands_set_variable() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_UTILS_API_KEY", "secret-value");
        let expanded = expand_env_var("${TEST_UTILS_API_KEY}");
        assert_eq!(expanded, "secret-value");
        std::env::remove_var("TEST_UTILS_API_KEY");
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
        std::env::set_var("TEST_UTILS_HOST", "example.com");
        let expanded = expand_env_var("https://${TEST_UTILS_HOST}/api");
        assert_eq!(expanded, "https://example.com/api");
        std::env::remove_var("TEST_UTILS_HOST");
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

    #[test]
    fn empty_string_input() {
        let expanded = expand_env_var("");
        assert_eq!(expanded, "");
    }

    #[test]
    fn multiple_embedded_references() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_UTILS_USER", "admin");
        std::env::set_var("TEST_UTILS_PASS", "s3cret");
        let expanded = expand_env_var("user=${TEST_UTILS_USER}&pass=${TEST_UTILS_PASS}");
        assert_eq!(expanded, "user=admin&pass=s3cret");
        std::env::remove_var("TEST_UTILS_USER");
        std::env::remove_var("TEST_UTILS_PASS");
    }

    #[test]
    fn adjacent_variables_no_separator() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_UTILS_A", "hello");
        std::env::set_var("TEST_UTILS_B", "world");
        let expanded = expand_env_var("${TEST_UTILS_A}${TEST_UTILS_B}");
        assert_eq!(expanded, "helloworld");
        std::env::remove_var("TEST_UTILS_A");
        std::env::remove_var("TEST_UTILS_B");
    }

    #[test]
    fn default_with_colon_in_value() {
        // The `:-` split is on the first occurrence, so "http://localhost:8080" stays intact.
        let expanded = expand_env_var("${SURELY_MISSING_UTILS:-http://localhost:8080}");
        assert_eq!(expanded, "http://localhost:8080");
    }

    #[test]
    fn set_variable_overrides_default() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_UTILS_OVERRIDE", "actual");
        let expanded = expand_env_var("${TEST_UTILS_OVERRIDE:-fallback}");
        assert_eq!(expanded, "actual");
        std::env::remove_var("TEST_UTILS_OVERRIDE");
    }

    #[test]
    fn unicode_in_default_value() {
        let expanded = expand_env_var("${NOPE_UTILS_UNICODE_42:-café résumé}");
        assert_eq!(expanded, "café résumé");
    }

    #[test]
    fn unicode_in_expanded_value() {
        // ALLOW: Mutex poisoning in tests is acceptable — propagates failure.
        #[allow(clippy::unwrap_used)]
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_UTILS_UNICODE", "日本語テスト");
        let expanded = expand_env_var("${TEST_UTILS_UNICODE}");
        assert_eq!(expanded, "日本語テスト");
        std::env::remove_var("TEST_UTILS_UNICODE");
    }

    #[test]
    fn dollar_sign_without_brace_is_literal() {
        let expanded = expand_env_var("price-is-$5.00");
        assert_eq!(expanded, "price-is-$5.00");
    }

    #[test]
    fn empty_variable_name_expands_to_empty() {
        // `${}` — the inner content is empty, so env::var("") returns an error → empty.
        let expanded = expand_env_var("${}");
        assert_eq!(expanded, "");
    }

    #[test]
    fn empty_default() {
        // `${VAR:-}` — default is explicitly empty.
        let expanded = expand_env_var("${SURELY_MISSING_UTILS_EMPTY:-}");
        assert_eq!(expanded, "");
    }

    #[rstest::rstest]
    #[case("", "")]
    #[case("plain", "plain")]
    #[case("${UNSET_UTILS_RSTEST}", "")]
    #[case("prefix-${UNSET_UTILS_RSTEST}", "prefix-")]
    #[case("${UNSET_UTILS_RSTEST}-suffix", "-suffix")]
    #[case("a${B}c${D}e", "ace")]
    fn parameterized_edge_cases(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(expand_env_var(input), expected);
    }
}
