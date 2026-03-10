@route-execution
Feature: Route Execution
  Execute selected routes with retry logic, fallback triggering on failures,
  and outcome recording for continuous learning.

  As a gateway operator
  I want requests executed with automatic failover and outcome tracking
  So that the system remains reliable and improves over time

  Rule: Primary route is executed first
    @smoke @critical
    Scenario: Successful primary route returns response
      Given a route decision with primary route "openai-gpt4"
      And the primary route responds successfully
      When the request is executed
      Then the response should be returned
      And the primary route should be recorded as successful

    @edge-case
    Scenario: Primary route timeout triggers fallback
      Given a route decision with primary route "openai-gpt4"
      And the primary route times out
      When the request is executed
      Then the fallback route should be attempted
      And the timeout should be recorded

  Rule: Retryable failures trigger fallback attempts
    @smoke @critical
    Scenario: Rate limit response triggers fallback
      Given a route decision with multiple fallbacks
      And the primary route returns rate limit error
      When the request is executed
      Then the first fallback should be attempted

    @regression
    Scenario: Server error triggers fallback
      Given a route decision with fallback routes
      And the primary route returns server error
      When the request is executed
      Then the fallback route should be attempted

    @edge-case
    Scenario: Non-retryable error does not trigger fallback
      Given a route decision with fallback routes
      And the primary route returns authentication error
      When the request is executed
      Then no fallback should be attempted
      And the error should be returned immediately

  Rule: Retry budget limits total attempts
    @critical
    Scenario: Retry budget exhausted returns failure
      Given a request with retry budget of 3
      And all routes fail with retryable errors
      When the request is executed
      Then exactly 3 attempts should be made
      And the final error should be returned

    @edge-case
    Scenario: Success within budget stops retrying
      Given a request with retry budget of 3
      And the second route succeeds
      When the request is executed
      Then only 2 attempts should be made
      And the successful response should be returned

  Rule: Loop guard prevents runaway execution
    @smoke @critical
    Scenario: Repeated same route triggers loop guard
      Given a request chain already attempted route "openai-gpt4"
      When the same route is selected again
      Then the loop guard should block the attempt
      And a different route should be selected

    @edge-case
    Scenario: Same provider repetition triggers diversity requirement
      Given two consecutive failures on provider "openai"
      When selecting the next route
      Then a different provider should be preferred

  Rule: Outcomes are recorded for all attempts
    @critical
    Scenario: Successful outcome recorded with metrics
      Given a route execution that succeeds
      When the outcome is recorded
      Then success should be recorded
      And latency should be captured
      And token usage should be stored

    @edge-case
    Scenario: Failed outcome recorded with error class
      Given a route execution that fails
      When the outcome is recorded
      Then failure should be recorded
      And error classification should be stored
      And fallback usage should be noted
