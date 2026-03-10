@health-management
Feature: Health Management
  Track credential health through state transitions, enable graceful degradation
  when services are unhealthy, and support recovery detection.

  As a gateway operator
  I want the system to detect unhealthy credentials and adapt routing behavior
  So that requests are not sent to failing services and recovery is automatic

  Rule: Health state transitions based on request outcomes
    @smoke @critical
    Scenario: Rate limit triggers degraded state
      Given a healthy credential
      When a rate limit response is received
      Then the credential should transition to degraded state

    @critical
    Scenario: Consecutive failures trigger unhealthy state
      Given a degraded credential
      When 5 consecutive failures occur
      Then the credential should transition to unhealthy state

    @edge-case
    Scenario: Success streak recovers degraded credential
      Given a degraded credential
      When 3 consecutive successes occur
      Then the credential should transition to healthy state

  Rule: Unhealthy credentials enter cooldown period
    @critical
    Scenario: Unhealthy credential blocked from selection
      Given an unhealthy credential in cooldown
      When routes are selected
      Then the unhealthy credential should not be considered

    @edge-case
    Scenario: Cooldown expiration allows recovery attempt
      Given an unhealthy credential with expired cooldown
      When routes are selected
      Then the credential should be considered for selection
      And state should transition to degraded

  Rule: Planner mode adapts to available data
    @smoke @critical
    Scenario: Full data enables learned mode
      Given sufficient route history exists
      When the planner selects a mode
      Then learned mode should be used

    @regression
    Scenario: Sparse data uses heuristic mode
      Given limited route history
      When the planner selects a mode
      Then heuristic mode should be used

    @edge-case
    Scenario: Missing state forces safe weighted mode
      Given the statistics store is unavailable
      When the planner selects a mode
      Then safe weighted mode should be used

    @edge-case
    Scenario: Critical failure uses deterministic fallback
      Given a planner internal error occurs
      When the system recovers
      Then deterministic fallback mode should be used
