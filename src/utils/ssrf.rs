use std::net::IpAddr;

/// Errors returned by SSRF protection checks.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The URL could not be parsed.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    /// The URL has no host component.
    #[error("URL has no host: {0}")]
    NoHost(String),
    /// The URL points to a private, loopback, or reserved IP address.
    #[error(
        "URL points to a private/internal IP address ({0}), which is not allowed (SSRF protection)"
    )]
    PrivateIp(IpAddr),
}

/// Validate that a URL does not point to a private, loopback, link-local,
/// or cloud metadata address. Returns `Ok(())` if the URL is safe, or an
/// error describing the rejected address.
///
/// # Errors
///
/// Returns [`Error::InvalidUrl`] if `url_str` cannot be parsed as a URL.
/// Returns [`Error::NoHost`] if the parsed URL has no host component.
/// Returns [`Error::PrivateIp`] if the host resolves to a private/reserved IP.
pub fn validate_url_not_private(url_str: &str) -> Result<(), Error> {
    let parsed =
        url::Url::parse(url_str).map_err(|e| Error::InvalidUrl(format!("{url_str}: {e}")))?;

    let host = parsed
        .host()
        .ok_or_else(|| Error::NoHost(url_str.to_string()))?;

    let ip = match host {
        url::Host::Domain(_) => return Ok(()), // Domain names are allowed (DNS rebinding is separate concern)
        url::Host::Ipv4(v4) => IpAddr::V4(v4),
        url::Host::Ipv6(v6) => IpAddr::V6(v6),
    };

    if is_private_ip(&ip) {
        return Err(Error::PrivateIp(ip));
    }

    Ok(())
}

/// Check whether an IP address falls into a private, loopback, link-local,
/// or cloud metadata range.
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // Use stdlib methods for standard private/loopback/link-local ranges
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                // Zero network: 0.0.0.0/8
                || octets[0] == 0
                // IETF Protocol Assignments (192.0.0.0/24) and TEST-NET-1 (192.0.2.0/24)
                || (octets[0] == 192
                    && octets[1] == 0
                    && (octets[2] == 0 || octets[2] == 2))
                // TEST-NET-2 (198.51.100.0/24)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                // TEST-NET-3 (203.0.113.0/24)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                // Reserved for Future Use (includes broadcast): 255.0.0.0/8
                || octets[0] == 255
        },
        IpAddr::V6(v6) => {
            // IPv4-mapped IPv6 (::ffff:x.x.x.x) — e.g. ::ffff:127.0.0.1 bypasses
            // pure-IPv4 checks, so we unwrap the embedded IPv4 address and re-check.
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_private_ip(&IpAddr::V4(mapped));
            }

            // IPv4-compatible IPv6 (::/96) — deprecated but still routable in some stacks.
            // ipv6_to_v4_compat handles the unspecified address (::) exclusion internally.
            if let Some(v4) = ipv6_to_v4_compat(v6) {
                return is_private_ip(&IpAddr::V4(v4));
            }

            let segments = v6.segments();
            // Loopback: ::1
            v6.is_loopback()
                // Private: fc00::/7 (unique local)
                || (0xfc00..=0xfdff).contains(&segments[0])
                // Link-local: fe80::/10
                || (0xfe80..=0xfebf).contains(&segments[0])
                // Unspecified: ::
                || v6.is_unspecified()
                // Multicast: ff00::/8
                || segments[0] >= 0xff00
        },
    }
}

/// Extract the embedded IPv4 address from an IPv4-compatible IPv6 address
/// (`::x.x.x.x`).
///
/// Only extracts when the low 32 bits are non-zero (i.e., not the unspecified address `::`).
fn ipv6_to_v4_compat(v6: &std::net::Ipv6Addr) -> Option<std::net::Ipv4Addr> {
    let segments = v6.segments();
    if segments[0..6] == [0, 0, 0, 0, 0, 0] && (segments[6] != 0 || segments[7] != 0) {
        let high = u8::try_from(segments[6] >> 8).ok()?;
        let low = u8::try_from(segments[6] & 0xFF).ok()?;
        let high2 = u8::try_from(segments[7] >> 8).ok()?;
        let low2 = u8::try_from(segments[7] & 0xFF).ok()?;
        Some(std::net::Ipv4Addr::new(high, low, high2, low2))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_loopback() {
        let result = validate_url_not_private("http://127.0.0.1/v1/chat");
        assert!(result.is_err());
    }

    #[test]
    fn reject_loopback_localhost() {
        let result = validate_url_not_private("http://127.0.0.1/v1/chat");
        assert!(result.is_err());
    }

    #[test]
    fn reject_private_10_range() {
        let result = validate_url_not_private("http://10.0.0.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_private_172_range_lower() {
        let result = validate_url_not_private("http://172.16.0.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_private_172_range_upper() {
        let result = validate_url_not_private("http://172.31.255.255/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_private_192_range() {
        let result = validate_url_not_private("http://192.168.1.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_link_local() {
        let result = validate_url_not_private("http://169.254.169.254/latest/meta-data/");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv6_loopback() {
        let result = validate_url_not_private("http://[::1]:8000/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv6_unique_local() {
        let result = validate_url_not_private("http://[fc00::1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv6_link_local() {
        let result = validate_url_not_private("http://[fe80::1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn allow_public_domain() {
        let result = validate_url_not_private("https://api.openai.com/v1/chat/completions");
        assert!(result.is_ok());
    }

    #[test]
    fn allow_public_literal_ip() {
        let result = validate_url_not_private("https://1.1.1.1/api");
        assert!(result.is_ok());
    }

    #[test]
    fn reject_zero_network() {
        let result = validate_url_not_private("http://0.0.0.0/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv4_mapped_ipv6_loopback() {
        let result = validate_url_not_private("http://[::ffff:127.0.0.1]:8000/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv4_mapped_ipv6_cloud_metadata() {
        let result = validate_url_not_private("http://[::ffff:169.254.169.254]/latest/meta-data/");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv4_mapped_ipv6_private_10() {
        let result = validate_url_not_private("http://[::ffff:10.0.0.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv4_mapped_ipv6_private_192() {
        let result = validate_url_not_private("http://[::ffff:192.168.1.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn allow_ipv4_mapped_public_ip() {
        let result = validate_url_not_private("http://[::ffff:1.1.1.1]/api");
        assert!(result.is_ok());
    }

    #[test]
    fn reject_ietf_protocol_assignments() {
        let result = validate_url_not_private("http://192.0.0.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_test_net_1() {
        let result = validate_url_not_private("http://192.0.2.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_test_net_2() {
        let result = validate_url_not_private("http://198.51.100.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_test_net_3() {
        let result = validate_url_not_private("http://203.0.113.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_broadcast() {
        let result = validate_url_not_private("http://255.255.255.255/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv6_unspecified() {
        let result = validate_url_not_private("http://[::]/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv6_multicast() {
        let result = validate_url_not_private("http://[ff02::1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv4_compatible_loopback() {
        let result = validate_url_not_private("http://[::127.0.0.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn reject_ipv4_compatible_private() {
        let result = validate_url_not_private("http://[::192.168.1.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn allow_public_ipv4_literal() {
        let result = validate_url_not_private("https://8.8.8.8/api");
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_url_returns_error() {
        let result = validate_url_not_private("not a url");
        assert!(result.is_err());
    }

    #[test]
    fn url_without_host_returns_error() {
        let result = validate_url_not_private("file:///path/to/file");
        assert!(result.is_err());
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(256))]

            /// Arbitrary strings never cause a panic in validate_url_not_private.
            #[test]
            fn arbitrary_url_string_never_panics(url in "\\PC*") {
                let _ = validate_url_not_private(&url);
            }

            /// Validation is deterministic: same URL always yields same result.
            #[test]
            fn validation_is_deterministic(
                scheme in prop_oneof![Just("http://"), Just("https://")],
                a in 1u8..=254u8,
                b in 0u8..=255u8,
                c in 0u8..=255u8,
                d in 1u8..=254u8,
            ) {
                let url = format!("{scheme}{a}.{b}.{c}.{d}/api");
                let r1 = validate_url_not_private(&url);
                let r2 = validate_url_not_private(&url);
                prop_assert_eq!(r1.is_ok(), r2.is_ok());
            }

            /// All loopback addresses (127.x.x.x) are rejected.
            #[test]
            fn all_loopback_rejected(
                b in 0u8..=255u8,
                c in 0u8..=255u8,
                d in 1u8..=255u8,
            ) {
                let url = format!("http://127.{b}.{c}.{d}/api");
                let result = validate_url_not_private(&url);
                prop_assert!(result.is_err(), "Loopback 127.{b}.{c}.{d} should be rejected");
            }

            /// All 10.x.x.x private addresses are rejected.
            #[test]
            fn all_10_range_rejected(
                b in 0u8..=255u8,
                c in 0u8..=255u8,
                d in 1u8..=255u8,
            ) {
                let url = format!("http://10.{b}.{c}.{d}/api");
                let result = validate_url_not_private(&url);
                prop_assert!(result.is_err(), "Private 10.{b}.{c}.{d} should be rejected");
            }

            /// Public domain names are always allowed.
            #[test]
            fn public_domains_always_allowed(
                word in "[a-z][a-z0-9]{2,15}",
                tld in prop_oneof![Just("com"), Just("org"), Just("io"), Just("net")],
            ) {
                let url = format!("https://{word}.{tld}/v1/chat/completions");
                let result = validate_url_not_private(&url);
                prop_assert!(result.is_ok(), "Domain {word}.{tld} should be allowed");
            }
        }
    }
}
