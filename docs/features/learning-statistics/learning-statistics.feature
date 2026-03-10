@learning-statistics
Feature: Learning and Statistics
  Aggregate route performance data, maintain statistical models for decision
  making, and enable continuous improvement through outcome recording.

  As a gateway operator
  I want the system to learn from traffic patterns and improve routing decisions
  So that performance and reliability increase over time without manual tuning

  Rule: Route statistics aggregate from execution outcomes
    @smoke @critical
    Scenario: Successful execution updates success count
      Given a route with existing statistics
      When a successful outcome is recorded
      Then the success count should increment
      And the last success timestamp should update

    @regression
    Scenario: Failed execution updates failure metrics
      Given a route with existing statistics
      When a timeout failure is recorded
      Then the timeout count should increment
      And average latency should be recalculated

    @edge-case
    Scenario: First execution creates initial statistics
      Given a route with no prior statistics
      When an outcome is recorded
      Then a new statistics entry should be created
      And all counters should be initialized

  Rule: Time-bucketed statistics enable contextual decisions
    @critical
    Scenario: Outcomes recorded in appropriate time bucket
      Given a request during peak hours
      When the outcome is recorded
      Then statistics should be aggregated under peak hour bucket

    @edge-case
    Scenario: Weekend traffic separated from weekday
      Given a request on Saturday
      When the outcome is recorded
      Then statistics should be aggregated under weekend bucket
      And not affect weekday averages

  Rule: Cold start uses inherited priors
    @smoke @critical
    Scenario: New route inherits provider prior
      Given a route for provider "anthropic" with no history
      And a prior for "anthropic" with 80% baseline success
      When the route is first considered
      Then the prior success rate should be 80%

    @edge-case
    Scenario: Tier-based prior when provider unknown
      Given a route for unknown provider
      And the model tier is "flagship"
      When the route is first considered
      Then flagship tier prior should be applied

    @edge-case
    Scenario: Neutral defaults when no prior exists
      Given a route with no provider or tier match
      When the route is first considered
      Then neutral 50% success prior should be used

  Rule: Attempt history enables decision tracing
    @critical
    Scenario: Route attempt recorded with decision context
      Given a route selection decision
      When the attempt is recorded
      Then the selected route should be logged
      And the selection mode should be captured
      And the predicted utility should be stored

    @edge-case
    Scenario: Fallback attempts linked to original request
      Given a request that tried three routes
      When attempt history is reviewed
      Then all three attempts should share the same request ID
      And the order should be preserved
