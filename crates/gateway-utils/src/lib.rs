//! Shared security and utility functions for the gateway.
//!
//! Provides timing-safe authentication, SSRF protection, and
//! environment variable expansion.

/// Environment variable expansion utilities.
pub mod env;
/// Timing-safe token comparison utilities.
pub mod security;
/// SSRF (Server-Side Request Forgery) protection.
pub mod ssrf;

pub use env::expand_env_var;
pub use security::constant_time_token_matches;
pub use ssrf::validate_url_not_private;
