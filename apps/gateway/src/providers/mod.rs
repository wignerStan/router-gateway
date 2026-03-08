//! Provider adapters for LLM API transformation
//!
//! This module provides adapters that transform between our gateway's
//! normalized format and provider-specific API formats.

pub mod anthropic;
pub mod openai;
pub mod google;
pub mod types;

pub use anthropic::AnthropicAdapter;
pub use openai::OpenAIAdapter;
pub use google::GoogleAdapter;
pub use types::{ProviderAdapter, ProviderRequest, ProviderResponse, ProviderError};
