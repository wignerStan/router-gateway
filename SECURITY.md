# Security Policy

## Supported Versions

| Version                | Supported |
| ---------------------- | --------- |
| Latest (`main` branch) | Yes       |

Only the latest version on the `main` branch receives security updates. Older versions are not actively maintained.

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in this project, please report it responsibly.

**Preferred method:** Use [GitHub Security Advisories](https://github.com/wignerStan/router-gateway/security/advisories/new) to submit a report privately.

**Alternative:** Open an issue with the `security` label.

Please include:

- A clear description of the vulnerability
- Steps to reproduce the issue
- The affected component (gateway, smart-routing, model-registry, tracing, cli)
- Any potential impact or attack vector
- If applicable, a suggested fix or mitigation

We will acknowledge receipt within 48 hours and aim to provide a resolution or timeline within 7 days.

## Security Model

This gateway is designed for **local development and self-hosted deployments**. Key security considerations:

- **Authentication**: Bearer token authentication is supported via `auth_tokens` in the configuration. If no tokens are configured, authentication is disabled.
- **API keys**: Provider API keys are stored in the configuration file. Use environment variable substitution (`${VAR_NAME}`) to avoid storing secrets in plain text.
- **SSRF**: Provider `base_url` values should be restricted to trusted endpoints.
- **Rate limiting**: Per-credential rate limits can be configured via `rate_limit` in the configuration.

## Security Best Practices for Deployment

1. **Enable authentication**: Always configure `auth_tokens` in production.
2. **Use environment variables**: Store API keys in environment variables, not in the config file directly.
3. **Restrict network access**: Bind to `127.0.0.1` instead of `0.0.0.0` when not exposing the gateway externally.
4. **Keep dependencies updated**: Run `cargo audit` regularly to check for known vulnerabilities.
5. **Review credentials**: Use per-credential `rate_limit` and `daily_quota` to prevent abuse.
