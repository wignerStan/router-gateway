@request-classification
Feature: Request Classification
  Transform raw API requests into normalized routing context for intelligent
  route selection. Classify requests by extracting capabilities, estimating
  token counts, detecting request format, and determining quality preferences.

  As a gateway operator
  I want requests to be automatically classified based on their content and format
  So that the routing system can make appropriate selections

  Rule: Content type determines vision capability requirement
    @smoke @critical
    Scenario: Image attachment requires vision support
      Given a chat request containing an image attachment
      When the request is classified
      Then vision capability should be required

    @edge-case
    Scenario: Text-only content does not require vision
      Given a chat request containing only text content
      When the request is classified
      Then vision capability should not be required

    @regression
    Scenario: Mixed content with images requires vision
      Given a request with both text and image content
      When the request is classified
      Then vision capability should be required

  Rule: Tool presence determines tool calling requirement
    @smoke @critical
    Scenario: Tool definitions require tool support
      Given a request containing tool function definitions
      When the request is classified
      Then tool capability should be required

    @edge-case
    Scenario: Absence of tools means no tool requirement
      Given a request with no tool definitions
      When the request is classified
      Then tool capability should not be required

    @edge-case
    Scenario: Empty tool array does not require tools
      Given a request with an empty tool list
      When the request is classified
      Then tool capability should not be required

  Rule: Streaming preference is extracted from request flags
    @smoke @critical
    Scenario: Explicit streaming enabled requires streaming support
      Given a request with streaming enabled
      When the request is classified
      Then streaming capability should be required

    @edge-case
    Scenario: Explicit streaming disabled does not require streaming
      Given a request with streaming disabled
      When the request is classified
      Then streaming capability should not be required

    @regression
    Scenario: Default behavior when streaming flag is absent
      Given a request without a streaming parameter
      When the request is classified
      Then streaming capability should not be required

  Rule: Reasoning capability is inferred from model hints and flags
    @critical
    Scenario: Reasoning flag explicitly enabled requires thinking support
      Given a request with reasoning enabled in parameters
      When the request is classified
      Then thinking capability should be required

    @edge-case
    Scenario: Model family hint suggests reasoning requirement
      Given a request targeting a reasoning-optimized model family
      When the request is classified
      Then thinking capability should be required

    @regression
    Scenario: Standard requests do not require thinking
      Given a request with no reasoning indicators
      When the request is classified
      Then thinking capability should not be required

  Rule: Request format is detected from provider signatures
    @smoke @critical
    Scenario: OpenAI format requests are identified by structure
      Given a request with OpenAI-compatible message format
      When the request is classified
      Then the format should be identified as OpenAI

    @edge-case
    Scenario: Anthropic format requests are recognized
      Given a request with Anthropic message format
      When the request is classified
      Then the format should be identified as Anthropic

    @regression
    Scenario: Gemini format requests are detected
      Given a request with Gemini message structure
      When the request is classified
      Then the format should be identified as Gemini

    @edge-case
    Scenario: Unknown format defaults to generic handling
      Given a request with unrecognized message structure
      When the request is classified
      Then the format should be identified as generic

  Rule: Token estimates are calculated from content size
    @critical
    Scenario: Large prompt requires high context capacity
      Given a request with a prompt containing 50000 tokens
      When the request is classified
      Then the estimated input tokens should be 50000
      And a large context window should be required

    @edge-case
    Scenario: Small prompt fits standard context
      Given a request with a prompt containing 100 tokens
      When the request is classified
      Then the estimated input tokens should be 100
      And a standard context window should suffice

    @regression
    Scenario: Total estimated tokens combines input and expected output
      Given a request with 1000 input tokens
      And an expected output of 500 tokens
      When the request is classified
      Then the total estimated tokens should be 1500
