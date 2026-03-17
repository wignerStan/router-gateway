//! E2E test entry point for the gateway crate.
//!
//! This file references the shared e2e utilities module and test cases
//! located under `tests/e2e/` in the workspace root via symlinks in
//! this test directory.
//!
//! All e2e tests are gated behind the `e2e` feature flag and will not
//! be compiled during normal `cargo test` runs.

// ALLOW: E2E test modules omit doc comments — test names are self-documenting.
// ALLOW: E2E tests use expect/panic for fail-fast behavior on setup or assertion failures.
#![allow(missing_docs, clippy::expect_used, clippy::panic)]

#[cfg(feature = "e2e")]
mod common;

#[cfg(feature = "e2e")]
mod podman_e2e_tests;
