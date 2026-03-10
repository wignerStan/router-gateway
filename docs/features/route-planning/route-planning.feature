@route-planning
Feature: Route Planning
  Select optimal routes for LLM requests through candidate construction,
  constraint filtering, utility estimation, and intelligent selection with
  fallback planning.

  As a gateway operator
  I want requests routed to the best available model and provider
  So that the system optimizes for capability, cost, latency, and reliability

  Rule: Routes are constructed from available credentials and models
    @smoke @critical
    Scenario: Valid model with available credentials creates route candidates
      Given a classified request for model "gpt-4"
      And credentials exist for provider "openai"
      When route candidates are built
      Then at least one route candidate should be created

    @edge-case
    Scenario: No matching credentials results in empty candidate list
      Given a classified request for model "unknown-model"
      And no credentials exist for that model
      When route candidates are built
      Then no route candidates should be available

    @regression
    Scenario: Multiple credentials create multiple candidates
      Given a classified request for model "claude-3"
      And credentials exist for both "anthropic" and "azure-anthropic"
      When route candidates are built
      Then two route candidates should be created

  Rule: Hard constraints filter out infeasible routes
    @smoke @critical
    Scenario: Capability mismatch filters route
      Given a request requiring vision capability
      And a route candidate for a non-vision model
      When constraints are applied
      Then the candidate should be rejected for capability mismatch

    @edge-case
    Scenario: Insufficient context window filters route
      Given a request requiring 100K context
      And a route candidate with 32K context limit
      When constraints are applied
      Then the candidate should be rejected for context overflow

    @edge-case
    Scenario: Disabled provider filters all its routes
      Given a route candidate for a disabled provider
      When constraints are applied
      Then the candidate should be rejected for provider disabled

    @regression
    Scenario: Tenant policy violation filters route
      Given a request from tenant "basic-tier"
      And a route candidate for premium-only model
      When constraints are applied
      Then the candidate should be rejected for policy violation

  Rule: Utility is estimated from route features
    @critical
    Scenario: High success rate increases utility estimate
      Given a route candidate with 95% historical success
      When utility is estimated
      Then the utility score should be high

    @edge-case
    Scenario: High latency decreases utility estimate
      Given a route candidate with 5000ms average latency
      When utility is estimated
      Then the utility score should be reduced

    @critical
    Scenario: Cost sensitivity affects utility weighting
      Given a budget-sensitive request
      And a high-cost route candidate
      When utility is estimated
      Then the utility score should be penalized for cost

  Rule: Route selection uses bandit policy for exploration
    @smoke @critical
    Scenario: Thompson sampling explores uncertain routes
      Given multiple feasible route candidates
      And limited historical data on some routes
      When a route is selected
      Then uncertain routes have a chance of selection

    @edge-case
    Scenario: Exploitation favors known high-utility routes
      Given multiple feasible route candidates
      And one route with consistently high success
      When a route is selected
      Then the high-success route is likely selected

    @edge-case
    Scenario: Diversity penalty avoids correlated routes
      Given candidates sharing the same provider
      When primary and fallback are selected
      Then fallbacks should prefer different providers

  Rule: Fallback plan provides ordered alternatives
    @smoke @critical
    Scenario: Primary selection produces ordered fallback list
      Given multiple feasible route candidates
      When a route decision is made
      Then a primary route should be selected
      And at least two fallback routes should be ordered

    @edge-case
    Scenario: Limited candidates produce minimal fallbacks
      Given only two feasible route candidates
      When a route decision is made
      Then a primary route should be selected
      And one fallback should be available

    @edge-case
    Scenario: Fallbacks prioritize different authentication
      Given multiple candidates for same provider
      When fallback routes are planned
      Then fallbacks should use different auth credentials

  Rule: Session provider visibility enables multi-turn conversation affinity
    @critical
    Scenario: New session establishes provider affinity
      Given a request with a new session identifier
      When routes are selected
      Then any provider may be chosen
      And the selected provider should be recorded for the session

    @smoke @critical
    Scenario: Existing session prefers same provider
      Given a request with existing session "session-abc"
      And session "session-abc" previously used provider "anthropic"
      When routes are selected
      Then provider "anthropic" should be preferred if healthy

    @edge-case
    Scenario: Session provider unhealthy triggers fallback selection
      Given a request with existing session "session-xyz"
      And session "session-xyz" previously used provider "openai"
      And provider "openai" is currently unhealthy
      When routes are selected
      Then a different provider should be selected
      And the session provider affinity should be updated

    @regression
    Scenario: Multi-turn conversation maintains provider visibility
      Given an ongoing conversation with session "multi-turn-1"
      And the conversation has 5 previous turns on provider "google"
      When the next route is planned
      Then provider "google" should receive selection bonus
      And conversation context should be preserved
