# vllm serve

## Overview

The `vllm serve` command starts an OpenAI-compatible API server for serving LLM completions via HTTP. By default, vLLM uses a **high-performance Rust Axum HTTP server** for request handling, which provides lower latency and higher throughput compared to the previous Python FastAPI implementation.

### Rust Router Architecture

The Rust router handles HTTP requests and communicates with vLLM engine cores via ZeroMQ:

```
┌──────────────┐      HTTP      ┌──────────────────┐    ZeroMQ    ┌─────────────────┐
│   Client     │ ◄────────────► │  Rust Axum       │ ◄──────────► │  vLLM Engine    │
│              │                │  HTTP Server     │              │  (Python)       │
└──────────────┘                │  • OpenAI API    │              │  • PagedAttention│
                                │  • Tokenization  │              │  • Model Exec   │
                                │    (PyO3)        │              │  • KV Cache     │
                                │  • Load Balancing│              │                 │
                                └──────────────────┘              └─────────────────┘
```

**Key Benefits:**
- Lower latency through Rust's zero-cost abstractions
- Higher throughput with true async parallelism (no GIL)
- OpenAI-compatible API endpoints
- PyO3 integration for Python tokenizer compatibility
- ZeroMQ-based communication with engine cores

For detailed architecture information, see [Rust Router Architecture](../design/rust_router_architecture.md).

## JSON CLI Arguments

--8<-- "docs/cli/json_tip.inc.md"

## Arguments

--8<-- "docs/generated/argparse/serve.inc.md"

## Rust Router Options

### Default Behavior

By default, `vllm serve` uses the Rust router for single-server deployments:

```bash
vllm serve Qwen/Qwen2.5-1.5B-Instruct
```

### Python Router Fallback

To use the Python FastAPI server instead, use the `--use-python-router` flag:

```bash
vllm serve Qwen/Qwen2.5-1.5B-Instruct --use-python-router
```

**When Python Router is Used Automatically:**

The system automatically falls back to the Python router in these scenarios:

1. **Multiple API Servers**: When `api_server_count > 1`, the Python router is used because the Rust router currently only supports single-instance deployments.

2. **Explicit Flag**: When `--use-python-router` is explicitly specified.

```bash
# Multiple API servers automatically use Python router
vllm serve Qwen/Qwen2.5-1.5B-Instruct --api-server-count 4

# Warning message will appear:
# "Rust router currently only supports single API server. 
#  api_server_count > 1 is requested, falling back to Python."
```

### Limitations

**Rust Router Limitations:**

- Only supports `api_server_count = 1` (single API server instance)
- Tokenization requires Python GIL (minimal performance impact)
- Some advanced features may still require Python router

For production deployments requiring multiple API servers, use the Python router or deploy multiple Rust router instances behind an external load balancer.
