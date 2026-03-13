/// Auth info for weight calculation
#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub id: String,
    pub priority: Option<i32>,
    pub quota_exceeded: bool,
    pub unavailable: bool,
    pub model_states: Vec<ModelState>,
}

/// Model state information
#[derive(Debug, Clone)]
pub struct ModelState {
    pub unavailable: bool,
    pub quota_exceeded: bool,
}

/// Data availability assessment for planner mode adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataAvailability {
    /// Full data available - all metrics populated with sufficient history
    Full,
    /// Sparse data - some metrics missing or insufficient history
    Sparse,
    /// Missing state - critical metrics unavailable
    Missing,
}

/// Planner mode for weight calculation adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlannerMode {
    /// Learned mode - use full weight calculation with all factors
    Learned,
    /// Heuristic mode - simplified calculation using available metrics
    Heuristic,
    /// Safe weighted mode - conservative defaults for missing state
    SafeWeighted,
    /// Deterministic fallback - predictable selection when errors occur
    Deterministic,
}
