//! Multi-dimensional routing policy configuration
//!
//! This module defines policy-based routing rules that combine multiple
//! classification dimensions for fine-grained credential/model selection.

mod matcher;
mod registry;
mod types;

#[cfg(test)]
mod matcher_tests;

#[cfg(test)]
mod tests;

pub mod templates;

pub use matcher::PolicyMatcher;
pub use registry::PolicyRegistry;
pub use types::*;
