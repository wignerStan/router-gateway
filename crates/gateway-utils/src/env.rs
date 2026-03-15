/// Expand environment variable references in a string.
/// Supports `${VAR_NAME}`, `${VAR_NAME:-default}`, and embedded references
/// e.g., "Bearer ${AUTH_KEY}" or "${HOST:-localhost}:${PORT}"
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

    #[test]
    fn expands_set_variable() {
        std::env::set_var("TEST_UTILS_API_KEY", "secret-value");
        let expanded = expand_env_var("${TEST_UTILS_API_KEY}");
        assert_eq!(expanded, "secret-value");
        std::env::remove_var("TEST_UTILS_API_KEY");
    }

    #[test]
    fn expands_with_default_when_unset() {
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
        let expanded = expand_env_var("${SURELY_NONEXISTENT_VAR_XYZ}");
        assert_eq!(expanded, "");
    }
}
