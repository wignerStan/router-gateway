# API Reference

> **Source:** Gateway HTTP endpoints
> **Last Updated:** 2026-03-13

This document provides complete reference information for the Gateway HTTP API endpoints.

## Overview

The Gateway exposes a RESTful HTTP API for model discovery, health monitoring, and routing recommendations. All endpoints return JSON responses.

| Endpoint                    | Method | Description                        |
| --------------------------- | ------ | ---------------------------------- |
| `GET /`                     | GET    | Service information                |
| `GET /health`               | GET    | Health check status                |
| `GET /api/models`           | GET    | List available models              |
| `GET /api/route`            | GET    | Get routing recommendation         |
| `POST /v1/chat/completions` | POST   | OpenAI-compatible chat completions |

---

## Endpoints

### GET /

Returns basic service information including version and status.

#### Request

```http
GET / HTTP/1.1
Host: localhost:3000
```

#### Response

```json
{
  "name": "Gateway API",
  "version": "0.1.0",
  "description": "Smart routing gateway for LLM requests",
  "features": ["Smart Routing", "Model Registry", "LLM Tracing", "Health Management"],
  "endpoints": {
    "health": "/health",
    "models": "/api/models",
    "route": "/api/route"
  }
}
```

#### Response Fields

| Field         | Type   | Description                       |
| ------------- | ------ | --------------------------------- |
| `name`        | string | Service name                      |
| `version`     | string | Service version (semver)          |
| `description` | string | Service description               |
| `features`    | array  | List of available features        |
| `endpoints`   | object | Map of feature names to API paths |

#### Status Codes

| Code | Description                            |
| ---- | -------------------------------------- |
| 200  | Success - Service information returned |

---

### GET /health

Returns the health status of the gateway and its dependencies.

#### Request

```http
GET /health HTTP/1.1
Host: localhost:3000
```

#### Response

```json
{
  "status": "healthy",
  "uptime_secs": 3600,
  "credential_count": 3,
  "healthy_count": 3,
  "degraded_count": 0,
  "unhealthy_count": 0
}
```

#### Response Fields

| Field              | Type    | Description                                                |
| ------------------ | ------- | ---------------------------------------------------------- |
| `status`           | string  | Overall health status (`healthy`, `degraded`, `unhealthy`) |
| `uptime_secs`      | integer | Gateway uptime in seconds                                  |
| `credential_count` | integer | Total number of registered credentials                     |
| `healthy_count`    | integer | Number of healthy credentials                              |
| `degraded_count`   | integer | Number of degraded credentials                             |
| `unhealthy_count`  | integer | Number of unhealthy credentials                            |

#### Status Codes

| Code | Description                                |
| ---- | ------------------------------------------ |
| 200  | Success - Service is healthy or degraded   |
| 503  | Service Unavailable - Service is unhealthy |

---

### GET /api/models

Returns a list of all available models from the model registry.

#### Request

```http
GET /api/models HTTP/1.1
Host: localhost:3000
Accept: application/json
```

#### Query Parameters

| Parameter  | Type   | Required | Default | Description                                      |
| ---------- | ------ | -------- | ------- | ------------------------------------------------ |
| `provider` | string | No       | -       | Filter by provider (e.g., `openai`, `anthropic`) |
| `type`     | string | No       | -       | Filter by model type (e.g., `chat`, `embedding`) |

#### Response

```json
{
  "models": [
    {
      "id": "gpt-4o",
      "provider": "openai",
      "capabilities": [],
      "context_window": 128000
    }
  ],
  "count": 1,
  "message": "Models loaded from configuration"
}
```

#### Response Fields

| Field                     | Type    | Description                        |
| ------------------------- | ------- | ---------------------------------- |
| `models`                  | array   | Array of model objects             |
| `models[].id`             | string  | Unique model identifier            |
| `models[].provider`       | string  | Provider name                      |
| `models[].capabilities`   | array   | List of supported capabilities     |
| `models[].context_window` | integer | Maximum context window size        |
| `count`                   | integer | Total number of models returned    |
| `message`                 | string  | Status message about model loading |

#### Status Codes

| Code | Description                                  |
| ---- | -------------------------------------------- |
| 200  | Success - Models list returned               |
| 500  | Internal Server Error - Registry unavailable |

---

### GET /api/route

Returns a routing recommendation based on the configured smart routing strategy.

#### Request

```http
GET /api/route?model=gpt-4 HTTP/1.1
Host: localhost:3000
Accept: application/json
```

#### Query Parameters

| Parameter  | Type    | Required | Default    | Description               |
| ---------- | ------- | -------- | ---------- | ------------------------- |
| `model`    | string  | Yes      | -          | Target model ID to route  |
| `strategy` | string  | No       | `weighted` | Override routing strategy |
| `priority` | integer | No       | `0`        | Request priority (0-10)   |

#### Response

```json
{
  "model": "gpt-4",
  "provider": "openai",
  "recommended_auth": "auth-primary-001",
  "strategy": "weighted",
  "confidence": 0.95,
  "alternatives": [
    {
      "auth_id": "auth-secondary-001",
      "confidence": 0.85
    },
    {
      "auth_id": "auth-backup-001",
      "confidence": 0.7
    }
  ],
  "metadata": {
    "latency_ms": 120,
    "success_rate": 0.98,
    "health_status": "healthy"
  }
}
```

#### Response Fields

| Field                       | Type    | Description                            |
| --------------------------- | ------- | -------------------------------------- |
| `model`                     | string  | Requested model ID                     |
| `provider`                  | string  | Model provider                         |
| `recommended_auth`          | string  | Recommended credential identifier      |
| `strategy`                  | string  | Routing strategy used                  |
| `confidence`                | float   | Confidence score (0.0-1.0)             |
| `alternatives`              | array   | Alternative credential recommendations |
| `alternatives[].auth_id`    | string  | Alternative credential identifier      |
| `alternatives[].confidence` | float   | Alternative confidence score           |
| `metadata`                  | object  | Additional routing metadata            |
| `metadata.latency_ms`       | integer | Average latency in milliseconds        |
| `metadata.success_rate`     | float   | Historical success rate (0.0-1.0)      |
| `metadata.health_status`    | string  | Credential health status               |

#### Status Codes

| Code | Description                                            |
| ---- | ------------------------------------------------------ |
| 200  | Success - Routing recommendation returned              |
| 400  | Bad Request - Missing required `model` parameter       |
| 404  | Not Found - Model not found in registry                |
| 503  | Service Unavailable - No healthy credentials available |

---

### POST /v1/chat/completions

OpenAI-compatible chat completions endpoint. Proxies requests to the selected provider after classifying the request and planning the optimal route.

#### Request

```http
POST /v1/chat/completions HTTP/1.1
Host: localhost:3000
Authorization: Bearer <token>
Content-Type: application/json

{
  "model": "gpt-4o",
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "stream": false
}
```

#### Status Codes

| Code | Description                             |
| ---- | --------------------------------------- |
| 200  | Success - Chat completion response      |
| 401  | Unauthorized - Missing or invalid token |
| 429  | Too Many Requests - Rate limit exceeded |

---

## Error Responses

All endpoints follow a consistent error response format:

```json
{
  "error": {
    "code": "MODEL_NOT_FOUND",
    "message": "Model 'invalid-model' not found in registry",
    "details": {
      "model_id": "invalid-model"
    }
  },
  "timestamp": "2025-02-17T12:00:00Z"
}
```

### Error Codes

| Code                     | HTTP Status | Description                          |
| ------------------------ | ----------- | ------------------------------------ |
| `BAD_REQUEST`            | 400         | Invalid request parameters           |
| `MODEL_NOT_FOUND`        | 404         | Requested model does not exist       |
| `NO_HEALTHY_CREDENTIALS` | 503         | No credentials available for routing |
| `INTERNAL_ERROR`         | 500         | Unexpected server error              |

---

## Content Types

| Content-Type       | Description                               |
| ------------------ | ----------------------------------------- |
| `application/json` | Default response format for all endpoints |

---

## Rate Limiting

API endpoints are rate-limited per IP address with a default limit of 60 requests per minute. When the limit is exceeded, the gateway returns a `429 Too Many Requests` response with a JSON error body:

```json
{
  "error": {
    "code": "RATE_LIMITED",
    "message": "Rate limit exceeded. Please retry later."
  }
}
```

Expired rate limit buckets are pruned periodically in the background. No rate limit response headers are sent.

---

## See Also

- [Configuration Reference](./configuration.md) - Smart routing configuration options
- [API Transformation](../API_TRANSFORMATION.md) - Format conversion architecture
