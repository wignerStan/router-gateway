# API Reference

> **Source:** Gateway HTTP endpoints
> **Last Updated:** 2025-02-17

This document provides complete reference information for the Gateway HTTP API endpoints.

## Overview

The Gateway exposes a RESTful HTTP API for model discovery, health monitoring, and routing recommendations. All endpoints return JSON responses.

| Endpoint          | Method | Description                |
| ----------------- | ------ | -------------------------- |
| `GET /`           | GET    | Service information        |
| `GET /health`     | GET    | Health check status        |
| `GET /api/models` | GET    | List available models      |
| `GET /api/route`  | GET    | Get routing recommendation |

---

## Endpoints

### GET /

Returns basic service information including version and status.

#### Request

```http
GET / HTTP/1.1
Host: localhost:8080
```

#### Response

```json
{
  "name": "gateway",
  "version": "0.1.0",
  "status": "running"
}
```

#### Response Fields

| Field     | Type   | Description                                               |
| --------- | ------ | --------------------------------------------------------- |
| `name`    | string | Service name                                              |
| `version` | string | Service version (semver)                                  |
| `status`  | string | Current service status (`running`, `stopped`, `degraded`) |

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
Host: localhost:8080
```

#### Response

**Healthy Response:**

```json
{
  "status": "healthy",
  "timestamp": "2025-02-17T12:00:00Z",
  "checks": {
    "model_registry": "healthy",
    "smart_routing": "healthy"
  }
}
```

**Degraded Response:**

```json
{
  "status": "degraded",
  "timestamp": "2025-02-17T12:00:00Z",
  "checks": {
    "model_registry": "healthy",
    "smart_routing": "degraded"
  },
  "message": "Some credentials are unavailable"
}
```

#### Response Fields

| Field       | Type   | Description                                                |
| ----------- | ------ | ---------------------------------------------------------- |
| `status`    | string | Overall health status (`healthy`, `degraded`, `unhealthy`) |
| `timestamp` | string | ISO 8601 timestamp of the health check                     |
| `checks`    | object | Individual component health statuses                       |
| `message`   | string | Optional message explaining degraded/unhealthy status      |

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
Host: localhost:8080
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
      "id": "gpt-4",
      "name": "GPT-4",
      "provider": "openai",
      "type": "chat",
      "context_window": 8192,
      "max_tokens": 4096,
      "capabilities": ["chat", "function_calling", "vision"]
    },
    {
      "id": "claude-3-opus",
      "name": "Claude 3 Opus",
      "provider": "anthropic",
      "type": "chat",
      "context_window": 200000,
      "max_tokens": 4096,
      "capabilities": ["chat", "function_calling", "vision", "thinking"]
    }
  ],
  "count": 2,
  "timestamp": "2025-02-17T12:00:00Z"
}
```

#### Response Fields

| Field                     | Type    | Description                                    |
| ------------------------- | ------- | ---------------------------------------------- |
| `models`                  | array   | Array of model objects                         |
| `models[].id`             | string  | Unique model identifier                        |
| `models[].name`           | string  | Human-readable model name                      |
| `models[].provider`       | string  | Provider name                                  |
| `models[].type`           | string  | Model type (`chat`, `embedding`, `completion`) |
| `models[].context_window` | integer | Maximum context window size                    |
| `models[].max_tokens`     | integer | Maximum output tokens                          |
| `models[].capabilities`   | array   | List of supported capabilities                 |
| `count`                   | integer | Total number of models returned                |
| `timestamp`               | string  | ISO 8601 timestamp                             |

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
Host: localhost:8080
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

API endpoints may be rate-limited. Rate limit headers are included in responses:

```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1708180800
```

---

## See Also

- [Configuration Reference](./configuration.md) - Smart routing configuration options
- [API Transformation](../API_TRANSFORMATION.md) - Format conversion architecture
