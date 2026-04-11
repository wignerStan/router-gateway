//! Weight calculation for credential scoring

/// Auth info for weight calculation
#[derive(Debug, Clone)]
pub struct AuthInfo {
    /// Unique credential identifier
    pub id: String,
    /// Credential priority (higher = preferred)
    pub priority: Option<i32>,
    /// Whether the credential has exceeded its quota
    pub quota_exceeded: bool,
    /// Whether the credential is unavailable
    pub unavailable: bool,
    /// Per-model state information
    pub model_states: Vec<ModelState>,
}

/// Model state information
#[derive(Debug, Clone)]
pub struct ModelState {
    /// Whether the model is unavailable
    pub unavailable: bool,
    /// Whether the model has exceeded its quota
    pub quota_exceeded: bool,
}

/// Data availability assessment for planner mode adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataAvailability {
    /// Full data available - all metrics populated with sufficient history
    Full,
    /// Sparse data - some metrics missing or insufficient history
    Sparse,
    /// Missing state - critical metrics unavailable
    Missing,
}

/// Planner mode for weight calculation adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerMode {
    /// Learned mode - use full weight calculation with all factors
    Learned,
    /// Heuristic mode - simplified calculation using available metrics
    Heuristic,
    /// Safe weighted mode - conservative defaults for missing state
    SafeWeighted,
    /// Deterministic fallback - predictable selection when errors occur
    Deterministic,
}
