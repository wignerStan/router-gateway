// Cucumber v0.20 BDD test harness for smart-routing
//
// This file provides the custom test harness (harness = false) required by
// cucumber. Step definitions mapping to the .feature files in docs/features/
// will be added in subsequent subtasks.
//
// Uses #[derive(World)] — NOT #[derive(WorldInit)] (removed in cucumber 0.14.0).

use cucumber::World;

/// Shared world state for BDD scenarios.
///
/// Step definitions will populate this with test fixtures as they are
/// implemented for the 5 .feature files:
/// - request-classification
/// - health-management
/// - route-planning
/// - route-execution
/// - learning-statistics
#[derive(World, Default)]
pub struct BddWorld;

/// Entry point for the cucumber BDD test runner.
///
/// The `run()` method reads .feature files and executes matching step definitions.
/// Step definitions will be added via `cucumber::{given, when, then}` macros
/// in subtask-3-2.
#[tokio::main]
async fn main() {
    // Step definitions and feature file wiring will be implemented in subtask-3-2.
    // The `run()` call requires at least one valid feature file path or glob.
    //
    // For now, this stub verifies that cucumber v0.20 compiles successfully
    // on Rust 1.85 (MSRV compatibility gate).
    //
    // Example of the final wiring:
    //   BddWorld::run("docs/features").await;
}
