# Getting Started with Gateway

Welcome! This tutorial will guide you through setting up and understanding the Gateway - a local LLM gateway with smart routing capabilities.

## Prerequisites

Before starting, ensure you have Rust installed:

```bash
# Check your Rust version (requires 1.85+)
rustc --version

# If not installed, use rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Quick Start

### 1. Build the Project

```bash
# Clone and enter the project
cd gateway

# Build all workspace members
cargo build
```

### 2. Run the Gateway

```bash
# Start the gateway server (listens on port 3000)
cargo run -p gateway
```

You should see output like:

```
INFO gateway: Model registry initialized
INFO gateway: Smart router initialized
INFO gateway: Gateway listening on 0.0.0.0:3000
```

## Understanding the Gateway

The Gateway is a Rust-based proxy that sits between your applications and multiple LLM providers. Its primary job is to intelligently route requests based on several factors:

| Feature            | Purpose                                            |
| ------------------ | -------------------------------------------------- |
| **Smart Routing**  | Selects the best credential/model for each request |
| **Model Registry** | Tracks available models and their capabilities     |
| **LLM Tracing**    | Records request/response data for analysis         |

## Your First Request

Open a new terminal and try these endpoints:

### Health Check

```bash
curl http://localhost:3000/health
# Response: {"status":"healthy","uptime_secs":0,"credential_count":0,"healthy_count":0,"degraded_count":0,"unhealthy_count":0}
```

### API Info

```bash
curl http://localhost:3000/
# Response: {"name":"Gateway API","version":"0.1.0",...}
```

### List Models

```bash
curl http://localhost:3000/api/models
# Response: {"models":[],"count":0,"message":"No models configured. Add credentials to gateway.yaml"}
```

### Route a Request

```bash
curl http://localhost:3000/api/route
# Response: {"classification":{"format":"chat","streaming":false,"estimated_tokens":0},"plan":{"primary":{"credential_id":"...","provider":"..."},"fallbacks":[]}}
```

## Understanding Routing Strategies

The smart routing system uses multiple strategies to select the optimal credential for each request. The router evaluates these weight factors:

| Factor       | Weight | Description                              |
| ------------ | ------ | ---------------------------------------- |
| Success Rate | 35%    | Historical reliability of the credential |
| Latency      | 25%    | Response time performance                |
| Health       | 20%    | Current endpoint health status           |
| Load         | 15%    | Current request load on the credential   |
| Priority     | 5%     | User-defined priority level              |

### Available Routing Strategies

1. **Weighted** - Balances across multiple factors
2. **Time-Aware** - Considers time-based patterns
3. **Quota-Aware** - Respects usage quotas and limits
4. **Adaptive** - Learns from request patterns over time

## Project Structure

```
gateway/
├── apps/
│   ├── gateway/      # Main HTTP server (Axum)
│   └── cli/          # Command-line interface
└── packages/
    ├── smart-routing/    # Routing algorithms
    ├── model-registry/   # Model management
    └── tracing/          # Request tracing
```

## Next Steps

Now that you have the gateway running:

1. **Add Credentials** - Configure your LLM provider API keys
2. **Explore Routing** - Learn how to customize routing strategies
3. **Enable Tracing** - Set up observability for your requests
4. **Deploy** - Run the gateway in production

For more details, see the [How-To Guides](../guides/) or dive into the [Reference](../reference/) documentation.
