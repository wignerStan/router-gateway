//! E2E test entry point for the gateway crate.
//!
//! This file references the shared e2e utilities module located at
//! `tests/e2e/common/mod.rs` in the workspace root. The actual e2e
//! test cases are added in subtask-4-2.
//!
//! All e2e tests are gated behind the `e2e` feature flag and will not
//! be compiled during normal `cargo test` runs.

#![allow(missing_docs, clippy::expect_used, clippy::panic)]

#[cfg(feature = "e2e")]
#[path = "../../../tests/e2e/common/mod.rs"]
mod common;
