//! Weight calculation for credential selection.
//!
//! Provides the [`WeightCalculator`] trait and [`DefaultWeightCalculator`] implementation
//! that computes credential weights based on success rate, latency, health, load, and priority.

mod calculator;
mod types;

#[cfg(test)]
mod tests;

pub use calculator::{DefaultWeightCalculator, WeightCalculator};
pub use types::{AuthInfo, ModelState};
