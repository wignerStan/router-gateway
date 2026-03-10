//! Provider adapters for LLM API transformation
//!
//! This module provides adapters that transform between our gateway's
//! normalized format and provider-specific API formats.

pub mod anthropic;
pub mod google;
pub mod openai;
pub mod types;

pub use anthropic::AnthropicAdapter;
pub use google::GoogleAdapter;
pub use openai::OpenAIAdapter;
pub use types::{ProviderAdapter, ProviderError, ProviderRequest, ProviderResponse};
