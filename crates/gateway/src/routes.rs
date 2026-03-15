//! Route handlers and Axum router construction

use axum::{extract::State, middleware::Next, Json};
use serde_json::{json, Value};
use std::net::SocketAddr;

use crate::config;
use crate::state::{AppState, HealthStatus, ModelInfo};
use smart_routing::classification::RequestClassifier;

pub async fn root() -> Json<Value> {
    Json(json!({
        "name": "Gateway API",
        "version": "0.1.0",
        "description": "Smart routing gateway for LLM requests",
        "features": [
            "Smart Routing",
            "Model Registry",
            "LLM Tracing",
            "Health Management"
        ],
        "endpoints": {
            "health": "/health",
            "models": "/api/models",
            "route": "/api/route"
        }
    }))
}

/// Authentication middleware for protected routes
/// Validates Bearer token against configured auth_tokens
/// Fails-closed by default (requires auth) unless GATEWAY_ENV=development is set
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    use axum::http::header::AUTHORIZATION;
    use axum::http::StatusCode;

    // Check for development environment override
    let is_development = std::env::var("GATEWAY_ENV")
        .map(|v| v.to_lowercase() == "development")
        .unwrap_or(false);

    // Skip auth only in development mode if no tokens are configured
    if is_development && state.config.server.auth_tokens.is_empty() {
        tracing::warn!("Authentication skipped in development mode (no auth_tokens configured)");
        return Ok(next.run(req).await);
    }

    // Fail-closed if no tokens are configured but we're not in development mode
    if state.config.server.auth_tokens.is_empty() {
        tracing::error!("Access denied: No auth_tokens configured in non-development mode");
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "type": "config_error",
                    "message": "Gateway is improperly configured: No authentication tokens available."
                }
            })),
        ));
    }

    // Extract Authorization header
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) => {
            // Check Bearer token format
            if let Some(token) = header.strip_prefix("Bearer ") {
                // Validate against configured tokens using constant-time comparison
                // that iterates all tokens (no short-circuit) to prevent timing side-channels
                if config::constant_time_token_matches(token, &state.config.server.auth_tokens) {
                    return Ok(next.run(req).await);
                }
            }
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "type": "invalid_request_error",
                        "message": "Invalid or expired API token"
                    }
                })),
            ))
        },
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "message": "Missing Authorization header. Use: Authorization: Bearer <token>"
                }
            })),
        )),
    }
}

/// Middleware that adds standard security headers to all HTTP responses.
pub async fn security_headers_middleware(
    req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    use axum::http::header::{HeaderName, HeaderValue};

    let mut response = next.run(req).await;

    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );

    response
}

/// Rate limiting middleware. Extracts client IP from X-Forwarded-For or
/// X-Real-IP headers only when `trust_proxy_headers` is enabled (gateway behind
/// a trusted reverse proxy). Otherwise, all requests share a single bucket to
/// prevent header-spoofing bypasses.
pub async fn rate_limit_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
    req: axum::extract::Request,
    next: Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    let peer_ip = addr.ip().to_string();
    let client_ip = if state.config.server.trust_proxy_headers {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()))
            .unwrap_or(&peer_ip)
    } else {
        &peer_ip
    };

    if !state.rate_limiter.check(client_ip) {
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "type": "rate_limit_error",
                    "message": "Too many requests. Please try again later."
                }
            })),
        ));
    }

    Ok(next.run(req).await)
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthStatus> {
    let uptime = state.start_time.elapsed().as_secs();

    // Count credential health states
    // For now, all configured credentials are considered healthy
    // In production, this would check actual health status from HealthManager
    let credential_count = state.credentials.len();
    let healthy_count = credential_count; // Assume all healthy until health checks run
    let degraded_count = 0;
    let unhealthy_count = 0;

    Json(HealthStatus {
        status: "healthy".to_string(),
        uptime_secs: uptime,
        credential_count,
        healthy_count,
        degraded_count,
        unhealthy_count,
    })
}

pub async fn list_models(State(state): State<AppState>) -> Json<Value> {
    // Build model list from configured credentials
    // Note: When allowed_models is empty, it means all provider models are allowed
    // In a full implementation, we would query the ModelRegistry for provider models
    let models: Vec<ModelInfo> = state
        .config
        .credentials
        .iter()
        .flat_map(|cred| {
            if cred.allowed_models.is_empty() {
                // Empty allowed_models means all models for this provider
                // TODO: Query ModelRegistry for all provider models
                vec![ModelInfo {
                    id: format!("{}:*", cred.provider),
                    provider: cred.provider.clone(),
                    capabilities: vec!["all".to_string()],
                    context_window: 128_000,
                }]
            } else {
                cred.allowed_models
                    .iter()
                    .map(|model_id| ModelInfo {
                        id: model_id.clone(),
                        provider: cred.provider.clone(),
                        capabilities: vec![], // Would be populated from model registry
                        context_window: 128_000, // Default, would come from registry
                    })
                    .collect()
            }
        })
        .collect();

    let count = models.len();

    Json(json!({
        "models": models,
        "count": count,
        "message": if count == 0 {
            "No models configured. Add credentials to gateway.yaml"
        } else {
            "Models loaded from configuration"
        }
    }))
}

pub async fn route_request(State(state): State<AppState>) -> Json<Value> {
    // Create a sample request for demonstration
    // In production, this would come from the request body as JSON
    let sample_request = json!({
        "messages": [
            {
                "role": "user",
                "content": "Hello, how are you?"
            }
        ],
        "model": "unknown"
    });

    // Step 1: Classify the request using RequestClassifier
    let classified = state.classifier.classify(&sample_request);

    // Step 2: Plan routes using Router with configured credentials
    let auths = state.credentials.clone();
    let session_id: Option<&str> = None;

    let route_plan = state.router.plan(&classified, auths, session_id).await;

    // Step 3: Return the route plan
    // In production, Step 4 would execute the route using RouteExecutor
    // and Step 5 would return the LLM response

    // Format the primary route
    let primary_json = match &route_plan.primary {
        Some(primary) => json!({
            "credential_id": primary.credential_id,
            "model_id": primary.model_id,
            "provider": primary.provider,
            "utility": primary.utility,
            "weight": primary.weight,
        }),
        None => json!(null),
    };

    // Format fallbacks
    let fallbacks_json: Vec<Value> = route_plan
        .fallbacks
        .iter()
        .map(|fb| {
            json!({
                "credential_id": fb.auth_id,
                "position": fb.position,
                "weight": fb.weight,
                "provider": fb.provider,
            })
        })
        .collect();

    // Build response
    Json(json!({
        "route_plan": {
            "primary": primary_json,
            "fallbacks": fallbacks_json,
            "total_candidates": route_plan.total_candidates,
            "filtered_candidates": route_plan.filtered_candidates,
        },
        "classification": {
            "required_capabilities": {
                "vision": classified.required_capabilities.vision,
                "tools": classified.required_capabilities.tools,
                "streaming": classified.required_capabilities.streaming,
                "thinking": classified.required_capabilities.thinking
            },
            "estimated_tokens": classified.estimated_tokens,
            "format": format!("{:?}", classified.format),
            "quality_preference": format!("{:?}", classified.quality_preference)
        },
        "message": if route_plan.primary.is_some() {
            "Route planned successfully"
        } else {
            "No suitable routes found - configure credentials in gateway.yaml"
        }
    }))
}

/// POST /v1/chat/completions - Proxy endpoint for chat completion requests
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    use crate::providers::{self, ProviderAdapter};

    // Step 1: Classify the request
    let classified = state.classifier.classify(&request);

    // Step 2: Extract model from request
    let model_id = request
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

    // Step 3: Plan routes
    let auths = state.credentials.clone();
    let session_id = request.get("session_id").and_then(|s| s.as_str());
    let route_plan = state.router.plan(&classified, auths, session_id).await;

    // Step 4: Get primary route
    let primary = match &route_plan.primary {
        Some(p) => p,
        None => {
            return Err((
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "type": "no_route_available",
                        "message": "No suitable routes found. Configure credentials in gateway.yaml"
                    }
                })),
            ));
        },
    };

    // Step 5: Find credential config for this route
    let credential = match state
        .config
        .credentials
        .iter()
        .find(|c| c.id == primary.credential_id)
    {
        Some(cred) => cred,
        None => {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "type": "credential_not_found",
                        "message": format!("Credential {} not found in configuration", primary.credential_id)
                    }
                })),
            ));
        },
    };

    // Step 6: Select provider adapter
    let provider = &primary.provider;
    let adapter: Box<dyn ProviderAdapter> = match provider.as_str() {
        "openai" | "azure-openai" => Box::new(providers::OpenAIAdapter::new()),
        "google" => Box::new(providers::GoogleAdapter::new()),
        "deepseek" => Box::new(providers::OpenAIAdapter::new()),
        "mistral" | "mistral-large" => Box::new(providers::OpenAIAdapter::new()),
        _ => Box::new(providers::OpenAIAdapter::new()), // Default to OpenAI format
    };

    // Step 7: Transform request for provider
    let _transformed = adapter.transform_request(&providers::types::ProviderRequest {
        messages: vec![], // Would parse from request
        model: model_id.to_string(),
        max_tokens: request
            .get("max_tokens")
            .and_then(|m| m.as_u64())
            .map(|v| v as u32),
        temperature: request
            .get("temperature")
            .and_then(|t| t.as_f64())
            .map(|v| v as f32),
        top_p: request
            .get("top_p")
            .and_then(|t| t.as_f64())
            .map(|v| v as f32),
        stop: request.get("stop").and_then(|s| s.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }),
        stream: request
            .get("stream")
            .and_then(|s| s.as_bool())
            .unwrap_or(false),
        system: request
            .get("system")
            .and_then(|s| s.as_str().map(String::from)),
        tools: None,
        tool_choice: None,
    });

    let endpoint = adapter.get_endpoint(credential.base_url.as_deref(), model_id);
    let _headers = adapter.build_headers(&credential.api_key);

    // For now, return a mock response (actual HTTP call would go here)
    // TODO: Implement actual upstream HTTP call with reqwest
    tracing::info!("Proxying request to {} at {}", provider, endpoint);

    // Return mock response for now
    // ALLOW: SystemTime before UNIX_EPOCH is impossible in practice.
    #[allow(clippy::expect_used)]
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();
    Ok(Json(json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": created,
        "model": model_id,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": format!("[Gateway mock response - route: {}, provider: {}]", primary.credential_id, provider)
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": classified.estimated_tokens,
            "completion_tokens": 50,
            "total_tokens": classified.estimated_tokens + 50
        },
        "_gateway": {
            "route": {
                "credential_id": primary.credential_id,
                "provider": provider,
                "utility": primary.utility,
            },
            "classification": {
                "format": format!("{:?}", classified.format),
                "capabilities": {
                    "vision": classified.required_capabilities.vision,
                    "tools": classified.required_capabilities.tools,
                    "streaming": classified.required_capabilities.streaming,
                }
            }
        }
    })))
}
