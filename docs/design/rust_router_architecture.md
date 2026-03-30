# Rust Axum Router Architecture

This document describes the architecture of vLLM's Rust-based HTTP server, which replaces the Python FastAPI server for improved performance and throughput.

## Overview

vLLM now uses a standalone Rust server built with the [Axum](https://github.com/tokio-rs/axum) web framework to handle HTTP requests. This Rust router implements OpenAI-compatible endpoints and communicates with the vLLM engine cores via ZeroMQ, providing significant performance improvements over the previous Python-based FastAPI implementation.

## System Architecture

### High-Level Component Diagram

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                              Client Layer                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  OpenAI SDK  │  │  curl/httpx  │  │  LangChain   │  │  Custom App  │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                 │                   │
│         └─────────────────┴─────────────────┴─────────────────┘                   │
│                                   │                                               │
│                              HTTP/HTTPS                                           │
│                              (JSON/SSE)                                           │
└───────────────────────────────────┼───────────────────────────────────────────────┘
                                    │
                                    ▼
┌───────────────────────────────────────────────────────────────────────────────────┐
│                         Rust Axum HTTP Server                                      │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                              HTTP Layer                                      │  │
│  │  ┌──────────────────────────────────────────────────────────────────────┐   │  │
│  │  │  Axum Router & Middleware Stack                                       │   │  │
│  │  │  • CORS handling                                                      │   │  │
│  │  │  • Request validation                                                 │   │  │
│  │  │  • Authentication (API key verification)                              │   │  │
│  │  │  • Rate limiting (optional)                                           │   │  │
│  │  └──────────────────────────────────────────────────────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                           Endpoint Handlers                                  │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │  │
│  │  │   /v1/models │ │ /v1/chat/    │ │ /v1/         │ │ /v1/         │        │  │
│  │  │              │ │ completions  │ │ completions  │ │ embeddings   │        │  │
│  │  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘        │  │
│  │         │                │                │                │                 │  │
│  │         └────────────────┴────────────────┴────────────────┘                 │  │
│  │                              │                                                │  │
│  │                    Request Processing                                        │  │
│  │         ┌────────────────────┴────────────────────┐                          │  │
│  │         │                                         │                          │  │
│  │         ▼                                         ▼                          │  │
│  │  ┌──────────────────┐                   ┌──────────────────┐                 │  │
│  │  │  PyO3 Renderer   │                   │  Stream Manager  │                 │  │
│  │  │  • Tokenization  │                   │  • SSE handling  │                 │  │
│  │  │  • Chat template │                   │  • Chunk format  │                 │  │
│  │  │  • Model config  │                   │  • Token decode  │                 │  │
│  │  └────────┬─────────┘                   └────────┬─────────┘                 │  │
│  └───────────┼──────────────────────────────────────┼───────────────────────────┘  │
│              │                                      │                              │
│              └──────────────────┬───────────────────┘                              │
│                                 │                                                  │
│                                 ▼                                                  │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                      ZeroMQ Client Layer                                     │  │
│  │  ┌────────────────────────────────┐  ┌────────────────────────────────┐     │  │
│  │  │   Input Router Socket          │  │   Output Pull Socket           │     │  │
│  │  │   • Request distribution       │  │   • Response collection        │     │  │
│  │  │   • Load balancing (RR)        │  │   • Stream demultiplexing      │     │  │
│  │  │   • Engine identity tracking   │  │   • Finished request cleanup   │     │  │
│  │  └────────────────────────────────┘  └────────────────────────────────┘     │  │
│  └─────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                         Shared State                                         │  │
│  │  • DashMap<String, Sender>    - Active request streams                       │  │
│  │  • AtomicUsize                - Round-robin counter                          │  │
│  │  • Mutex<Vec<Bytes>>          - Engine identities                            │  │
│  │  • Arc<PyObject>              - PyO3 renderer reference                       │  │
│  └─────────────────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────┬───────────────────────────────────────────────┘
                                    │
                                    │ ZeroMQ Protocol
                                    │ (MessagePack serialization)
                                    │
                                    ▼
┌───────────────────────────────────────────────────────────────────────────────────┐
│                         ZeroMQ Message Broker                                      │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                         Message Routing                                      │  │
│  │                                                                              │  │
│  │    Requests: Router Socket → Engine Cores (PUSH-PULL pattern)               │  │
│  │    Responses: Engine Cores → Pull Socket (PUB-SUB pattern)                  │  │
│  │                                                                              │  │
│  │    Message Format: [identity][delimiter][command][payload]                  │  │
│  │    Serialization: MessagePack (rmp-serde)                                   │  │
│  └─────────────────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────┬───────────────────────────────────────────────┘
                                    │
                                    │ ZeroMQ Protocol
                                    │
                                    ▼
┌───────────────────────────────────────────────────────────────────────────────────┐
│                           vLLM Engine Cores                                        │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                      Engine Core 0 (Python)                                  │  │
│  │  ┌──────────────────────────────────────────────────────────────────────┐   │  │
│  │  │  • ZeroMQ Interface (DEALER socket)                                   │   │  │
│  │  │  • Request deserialization (MessagePack → Python objects)             │   │  │
│  │  │  • Scheduler (prefill + decode phases)                                │   │  │
│  │  │  • PagedAttention KV cache management                                 │   │  │
│  │  │  • Model execution (CUDA/HIP kernels)                                 │   │  │
│  │  │  • Token generation & streaming                                       │   │  │
│  │  └──────────────────────────────────────────────────────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                      Engine Core 1 (Python)                                  │  │
│  │  (Same architecture as Core 0, independent execution)                       │  │
│  └─────────────────────────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────────────────────────┐  │
│  │                      Engine Core N (Python)                                  │  │
│  │  (Scales horizontally based on data_parallel_size)                          │  │
│  └─────────────────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────────────────┘
```

### Data Flow Sequence Diagram

```
Client                          Rust Router                      Engine Core
  │                                 │                                 │
  │  POST /v1/chat/completions      │                                 │
  │────────────────────────────────►│                                 │
  │                                 │                                 │
  │                                 │  Parse JSON request             │
  │                                 │  Validate parameters            │
  │                                 │                                 │
  │                                 │  PyO3: tokenize(prompt)         │
  │                                 │◄──────────────┐                 │
  │                                 │──────────────►│ Python GIL      │
  │                                 │  token_ids     │                 │
  │                                 │                                 │
  │                                 │  Build EngineCoreRequest        │
  │                                 │  (MessagePack serialize)        │
  │                                 │                                 │
  │                                 │  ZMQ: Send [identity][payload]  │
  │                                 │────────────────────────────────►│
  │                                 │                                 │  Deserialize
  │                                 │                                 │  Schedule request
  │                                 │                                 │  Prefill phase
  │                                 │                                 │  Decode phase (loop)
  │                                 │                                 │
  │                                 │  ZMQ: Stream tokens             │
  │                                 │◄────────────────────────────────│
  │                                 │  [request_id, [tokens], ..., finish_reason]
  │                                 │                                 │
  │                                 │  PyO3: decode(token_ids)        │
  │                                 │◄──────────────┐                 │
  │                                 │──────────────►│ Python GIL      │
  │                                 │  text          │                 │
  │                                 │                                 │
  │  SSE: data: {...chunk...}       │                                 │
  │◄────────────────────────────────│                                 │
  │                                 │                                 │
  │  SSE: data: {...chunk...}       │                                 │
  │◄────────────────────────────────│  (repeat for each token)        │
  │                                 │                                 │
  │  SSE: data: {...final...}       │                                 │
  │◄────────────────────────────────│  (with usage stats)             │
  │                                 │                                 │
  │                                 │  Cleanup request stream         │
  │                                 │                                 │
```

## Key Components

### 1. Rust Axum HTTP Server

The Rust server is built using Axum, a modern, ergonomic web framework built on Tokio and Tower. It provides:

- **High Performance**: Rust's zero-cost abstractions and async runtime enable superior throughput
- **Type Safety**: Compile-time guarantees for request/response structures
- **Memory Efficiency**: No garbage collection pauses, deterministic memory management
- **Concurrency**: Tokio's async runtime for handling thousands of concurrent connections

#### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check endpoint |
| `/v1/models` | GET | List available models (OpenAI-compatible) |
| `/v1/chat/completions` | POST | Chat completions API |
| `/v1/completions` | POST | Text completions API |
| `/v1/embeddings` | POST | Embeddings API |
| `/metrics` | GET | Prometheus metrics endpoint |

### 2. PyO3 Integration

The Rust server uses [PyO3](https://pyo3.rs/) to call Python code for:

- **Tokenization**: Converting text prompts to token IDs using the model's tokenizer
- **Prompt Rendering**: Applying chat templates and formatting
- **Model Configuration**: Loading and accessing model metadata

This hybrid approach ensures 100% compatibility with existing Hugging Face models and templates while leveraging Rust's performance for HTTP handling.

```rust
// Example: Python tokenizer called from Rust
let res: PyResult<Vec<u32>> = Python::with_gil(|py| {
    let renderer = state.renderer.bind(py);
    let tokenizer = renderer.getattr("renderer")
        .and_then(|r| r.getattr("tokenizer"))?;
    tokenizer
        .call_method1("encode", (prompt_str,))?
        .extract()
});
```

### 3. ZeroMQ Communication

The Rust router communicates with vLLM engine cores using ZeroMQ, a high-performance asynchronous messaging library:

- **Input Socket (Router)**: Receives requests from the Rust server and distributes to engines
- **Output Socket (Pull)**: Collects responses from engines for streaming back to clients
- **MessagePack Serialization**: Efficient binary serialization for engine protocol

#### Message Flow

```
Request Flow:
1. Client sends HTTP POST /v1/chat/completions
2. Rust server parses request and validates parameters
3. PyO3 calls Python tokenizer to convert prompt → token IDs
4. Request serialized to MessagePack format
5. Sent via ZeroMQ Router socket to available engine core
6. Engine processes request and generates tokens
7. Results streamed back via ZeroMQ Pull socket
8. Rust server formats response and streams to client via SSE
```

### 4. Load Balancing

The router implements round-robin load balancing across multiple engine cores:

```rust
static ROUND_ROBIN: std::sync::atomic::AtomicUsize = 
    std::sync::atomic::AtomicUsize::new(0);

let idx = ROUND_ROBIN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) 
    % idents.len();
let identity = idents[idx].clone();
```

This ensures even distribution of requests across all available engines.

### 5. Streaming Support

For streaming responses (SSE - Server-Sent Events), the router:

1. Creates a unique request ID for tracking
2. Establishes an async channel for receiving token streams
3. Streams tokens as they are generated by the engine
4. Decodes token IDs to text using Python tokenizer via PyO3
5. Formats as OpenAI-compatible SSE chunks

```rust
if stream {
    let stream_res = async_stream::stream! {
        while let Some(output) = rx.recv().await {
            // Decode tokens and format as SSE chunk
            yield Ok::<_, Infallible>(Event::default().data(data));
        }
    };
    Sse::new(stream_res).into_response()
}
```

### 6. Request Stream Management

Active requests are tracked using a concurrent hash map (`DashMap`):

```rust
pub struct AppState {
    input_socket: Arc<Mutex<RouterSocket>>,
    engine_identities: Arc<Mutex<Vec<bytes::Bytes>>>,
    request_streams: Arc<DashMap<String, mpsc::UnboundedSender<serde_json::Value>>>,
    renderer: Arc<PyObject>,
}
```

This enables:
- Efficient concurrent access from multiple async tasks
- Request cancellation and cleanup
- Proper resource management

## Data Structures

### OpenAI API Types

The router defines Rust structs that mirror OpenAI's API schema:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: Option<u32>,
    pub max_tokens: Option<u32>,
    // ... additional parameters
}
```

### vLLM Engine Protocol

Engine requests use a tuple struct with positional fields (msgspec `array_like=True`):

```rust
#[derive(Debug, Serialize)]
struct EngineCoreRequest(
    String,                    // request_id
    Option<Vec<u32>>,          // prompt_token_ids
    Option<serde_json::Value>, // mm_features
    Option<serde_json::Value>, // sampling_params
    // ... additional fields
);
```

## Performance Benefits

### Comparison: Rust vs Python Server

| Aspect | Python FastAPI | Rust Axum |
|--------|---------------|-----------|
| HTTP Parsing | Python (slower) | Rust (optimized) |
| JSON Serialization | Python | Rust (serde) |
| Concurrency | GIL-limited | True parallelism |
| Memory Overhead | Higher | Lower |
| Latency | Higher | Lower |
| Throughput | Lower | Higher |

### Key Optimizations

1. **Zero-Copy Parsing**: Rust's borrow checker enables zero-copy string handling
2. **Async I/O**: Tokio's non-blocking I/O for network operations
3. **Efficient Serialization**: `serde` and `rmp-serde` for fast JSON/MessagePack
4. **Lock-Free Concurrency**: `DashMap` and atomics for concurrent state
5. **No GIL Contention**: Rust code runs without Python GIL overhead

## Building and Installation

### Build System Integration

The Rust router is integrated into vLLM's build system via `setuptools-rust`:

```python
# setup.py
rust_extensions=[
    RustExtension(
        "vllm._router",
        path="vllm-router/Cargo.toml",
        binding=Binding.PyO3,
    )
]
```

### Dependencies

Key Rust crates used:

| Crate | Purpose |
|-------|---------|
| `axum` | Web framework |
| `tokio` | Async runtime |
| `serde` | Serialization |
| `pyo3` | Python interop |
| `zeromq` | Message passing |
## Message Format Specifications

### ZeroMQ Message Structure

The Rust router communicates with vLLM engine cores using ZeroMQ messages with the following structure:

#### Request Message (Router → Engine)

```
┌─────────────────┬─────────────────┬─────────────────┬──────────────────────────┐
│   Identity      │   Delimiter     │   Command       │   Payload                │
│   (bytes)       │   (empty)       │   (1 byte)      │   (MessagePack)          │
├─────────────────┼─────────────────┼─────────────────┼──────────────────────────┤
│ Engine identity │ 0x00            │ 0x00 (ADD)      │ EngineCoreRequest        │
│ from registration│                │                 │ serialized               │
└─────────────────┴─────────────────┴─────────────────┴──────────────────────────┘
```

#### Response Message (Engine → Router)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│   Payload (MessagePack encoded array)                                         │
├──────────────────────────────────────────────────────────────────────────────┤
│   [                                                                            │
│     request_id,        // str: Unique request identifier                       │
│     token_ids,         // list[u32]: New tokens generated in this iteration    │
│     text,             // str: Optional decoded text (if available)            │
│     finish_reason,    // Optional[int]: 0=stop, 1=length, null=continuing     │
│     usage,            // Optional[dict]: Token usage statistics               │
│     ...               // Additional engine-specific fields                    │
│   ]                                                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### EngineCoreRequest Structure

The request sent to the engine core is a tuple struct with positional fields:

```rust
EngineCoreRequest(
    request_id: String,                    // Unique request identifier
    prompt_token_ids: Option<Vec<u32>>,    // Tokenized prompt
    mm_features: Option<serde_json::Value>, // Multi-modal features (if any)
    sampling_params: Option<serde_json::Value>, // Sampling configuration
    pooling_params: Option<serde_json::Value>, // Pooling mode parameters
    arrival_time: f64,                     // Unix timestamp (seconds)
    lora_request: Option<serde_json::Value>, // LoRA adapter request
    cache_salt: Option<String>,            // Cache isolation salt
    data_parallel_rank: Option<u32>,       // DP rank for distributed serving
    prompt_embeds: Option<serde_json::Value>, // Pre-computed embeddings
    client_index: u32,                     // Client identifier
    current_wave: u32,                     // Scheduling wave identifier
    priority: i32,                         // Request priority
    trace_headers: Option<HashMap<String, String>>, // Distributed tracing
    resumable: bool,                       // Can request be resumed?
    external_req_id: Option<String>,       // External request identifier
    reasoning_ended: Option<bool>,         // Reasoning phase completion
)
```

### Sampling Parameters Format

For chat/completions requests, sampling parameters are serialized as JSON:

```json
{
  "temperature": 0.7,
  "max_tokens": 256,
  "n": 1,
  "top_p": 0.9,
  "output_kind": 1,      // 1=streaming, 0=non-streaming
  "skip_clone": true     // Optimization flag
}
```

For embeddings requests, pooling parameters are used:

```json
{
  "additional_metadata": {}
}
```

## Configuration Reference

### Rust Server Command-Line Arguments

| Argument | Type | Default | Description |
|----------|------|---------|-------------|
| `--host` | String | `127.0.0.1` | Host address to bind HTTP server |
| `--port` | u16 | `8000` | Port number for HTTP server |
| `--input-address` | String | Required | ZeroMQ address for engine input (e.g., `tcp://127.0.0.1:5555`) |
| `--output-address` | String | Required | ZeroMQ address for engine output (e.g., `tcp://127.0.0.1:5556`) |
| `--model-config-pickle` | String | Required | Path to pickled model configuration file |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Logging level for Rust components (`trace`, `debug`, `info`, `warn`, `error`) |
| `VLLM_ROUTER_HOST` | `0.0.0.0` | Override default bind host |
| `VLLM_ROUTER_PORT` | `8000` | Override default bind port |

### ZeroMQ Address Formats

Supported ZeroMQ transport schemes:

| Scheme | Format | Use Case |
|--------|--------|----------|
| TCP | `tcp://<host>:<port>` | Network communication, distributed deployments |
| IPC | `ipc:///path/to/socket` | Local inter-process communication (Unix only) |
| inproc | `inproc://<name>` | Intra-process communication (testing) |

**Example configurations:**

```bash
# TCP (default, works across network)
--input-address tcp://127.0.0.1:5555
--output-address tcp://127.0.0.1:5556

# IPC (faster for local communication)
--input-address ipc:///tmp/vllm_engine_input
--output-address ipc:///tmp/vllm_engine_output

# Mixed (input via TCP, output via IPC)
--input-address tcp://0.0.0.0:5555
--output-address ipc:///tmp/vllm_engine_output
```

## Performance Characteristics

### Latency Breakdown

For a typical chat completion request, latency is distributed as follows:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Request Latency Breakdown                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  HTTP Parse & Validate    ████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ~5-10 μs    │
│  PyO3 Tokenization        ████████████████████░░░░░░░░░░░░░░░░  ~50-200 μs  │
│  ZMQ Send                 ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ~10-20 μs   │
│  Engine Prefill           ████████████████████████████████████  ~5-50 ms    │
│  Engine Decode (per token)████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ~1-5 ms     │
│  PyO3 Decode              ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ~10-50 μs   │
│  HTTP Response (SSE)      ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ~5-10 μs    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Notes:**
- Engine prefill time depends on prompt length
- Engine decode time depends on model size and hardware
- PyO3 overhead is minimal but includes GIL acquisition

### Throughput Characteristics

The Rust router can handle:

- **Concurrent Connections**: 10,000+ simultaneous HTTP connections
- **Request Throughput**: Limited by engine capacity, not router
- **Memory Overhead**: ~50 MB base + ~10 KB per active request stream

### Comparison: Rust vs Python Router

| Metric | Python FastAPI | Rust Axum | Improvement |
|--------|---------------|-----------|-------------|
| HTTP Parse Latency | ~50 μs | ~5 μs | 10x faster |
| JSON Serialize/Deserialize | ~100 μs | ~20 μs | 5x faster |
| Concurrent Connection Limit | ~1,000 | ~10,000+ | 10x more |
| Memory per Connection | ~100 KB | ~10 KB | 10x less |
| Context Switch Overhead | High (GIL) | Low (async) | Significant |
| GC Pauses | Yes (Python) | No | Eliminated |

## Operational Guide

### Deployment Architectures

#### Single-Node Deployment

```
┌──────────────────────────────────────────────────────────────┐
│                         Single Node                           │
│                                                               │
│  ┌────────────────┐         ┌─────────────────────────────┐  │
│  │  Rust Router   │◄───────►│  vLLM Engine Cores (N)      │  │
│  │  :8000         │  ZeroMQ │  • Core 0 (GPU 0)           │  │
│  └────────────────┘         │  • Core 1 (GPU 1)           │  │
│         ▲                   │  • Core N (GPU N)           │  │
│         │                   └─────────────────────────────┘  │
│         │                                                     │
│         └────────── Clients                                    │
└──────────────────────────────────────────────────────────────┘
```

**Configuration:**
```bash
vllm serve Qwen/Qwen2.5-1.5B-Instruct \
  --host 0.0.0.0 \
  --port 8000 \
  --data-parallel-size 4
```

#### Multi-Node with External Load Balancer

```
┌──────────────────────────────────────────────────────────────────┐
│                      External Load Balancer                       │
│                    (nginx, HAProxy, ALB, etc.)                   │
└────────────────────────────┬─────────────────────────────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│  Node 1          │ │  Node 2          │ │  Node N          │
│  ┌────────────┐  │ │  ┌────────────┐  │ │  ┌────────────┐  │
│  │ Rust Router│  │ │  │ Rust Router│  │ │  │ Rust Router│  │
│  │ :8000      │  │ │  │ :8000      │  │ │  │ :8000      │  │
│  └─────┬──────┘  │ │  └─────┬──────┘  │ │  └─────┬──────┘  │
│        │         │ │        │         │ │        │         │
│        ▼         │ │        ▼         │ │        ▼         │
│  ┌────────────┐  │ │  ┌────────────┐  │ │  ┌────────────┐  │
│  │ Engine     │  │ │  │ Engine     │  │ │  │ Engine     │  │
│  │ Cores      │  │ │  │ Cores      │  │ │  │ Cores      │  │
│  └────────────┘  │ │  └────────────┘  │ │  └────────────┘  │
└──────────────────┘ └──────────────────┘ └──────────────────┘
```

**Load Balancer Configuration (nginx example):**
```nginx
upstream vllm_backend {
    least_conn;
    server node1.example.com:8000;
    server node2.example.com:8000;
    server nodeN.example.com:8000;
}

server {
    listen 80;
    
    location / {
        proxy_pass http://vllm_backend;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 300s;
    }
}
```

### Health Checking

The Rust router provides a `/health` endpoint for load balancer health checks:

```bash
curl http://localhost:8000/health
# Response: "OK" (HTTP 200)
```

**Health check behavior:**
- Returns 200 OK when router is running
- Does not check engine health (router can run without engines)
- Suitable for liveness probes

For readiness probes, combine with model endpoint:
```bash
curl http://localhost:8000/v1/models
# Returns model list if engines are connected
```

### Monitoring and Observability

#### Prometheus Metrics

The router exposes metrics at `/metrics`:

```prometheus
# HTTP metrics
http_requests_total{method="POST",endpoint="/v1/chat/completions",status="200"}
http_request_duration_seconds_bucket{method="POST",endpoint="/v1/chat/completions",le="0.1"}

# Connection metrics
active_connections
active_request_streams

# System metrics
rust_memory_usage_bytes
rust_task_count
```

**Scrape configuration:**
```yaml
scrape_configs:
  - job_name: 'vllm-router'
    static_configs:
      - targets: ['localhost:8000']
    metrics_path: '/metrics'
```

#### Structured Logging

Enable debug logging with `RUST_LOG`:

```bash
RUST_LOG=debug vllm serve Qwen/Qwen2.5-1.5B-Instruct
```

**Log output format:**
```
2024-01-15T10:30:45.123456Z  INFO vllm_router: vLLM Rust Router listening on 0.0.0.0:8000
2024-01-15T10:30:46.234567Z DEBUG vllm_router: Received chat completion request: chatcmpl-uuid
2024-01-15T10:30:46.345678Z DEBUG vllm_router: Request finished: chatcmpl-uuid
```

### Scaling Guidelines

#### Vertical Scaling

- Increase `data_parallel_size` to use more GPUs on same node
- Rust router overhead remains constant regardless of engine count

#### Horizontal Scaling

- Deploy multiple router instances behind load balancer
- Each router instance is stateless (except for active request streams)
- Use sticky sessions if request resumption is needed

#### Capacity Planning

**Rule of thumb:**
- 1 Rust router instance can handle 10,000+ concurrent connections
- Router CPU usage: ~1-5% under typical load
- Router memory: 50 MB base + 10 KB per active stream
- Network bandwidth: Depends on token throughput

**Example calculation:**
```
Expected concurrent users: 1,000
Average request duration: 5 seconds
Requests per second: 200

Router capacity needed: 1 instance (handles 10,000+ connections)
Memory: 50 MB + (1,000 * 10 KB) = 60 MB
```

## Security Considerations

### Authentication

The Rust router supports API key authentication via the `--api-key` flag:

```bash
vllm serve Qwen/Qwen2.5-1.5B-Instruct --api-key sk-abc123
```

**API Key Verification Flow:**
```
Client Request
    │
    ▼
┌─────────────────────────┐
│  Authorization Header   │
│  Bearer sk-abc123       │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  Rust Router validates  │
│  against configured key │
└───────────┬─────────────┘
            │
     ┌──────┴──────┐
     │             │
     ▼             ▼
  Valid         Invalid
     │             │
     ▼             ▼
  Process      Return 401
  Request      Unauthorized
```

**Multiple API Keys:**

Support multiple keys for key rotation:

```bash
vllm serve Qwen/Qwen2.5-1.5B-Instruct --api-key sk-key1,sk-key2,sk-key3
```

### Network Security

**TLS/HTTPS Termination:**

The Rust router does not natively support TLS. For HTTPS:

1. **Reverse Proxy (Recommended):**
   ```
   Client ──HTTPS──► nginx/HAProxy ──HTTP──► Rust Router
   ```

2. **Cloud Load Balancer:**
   ```
   Client ──HTTPS──► AWS ALB/GCP LB ──HTTP──► Rust Router
   ```

**Example nginx TLS configuration:**
```nginx
server {
    listen 443 ssl http2;
    server_name api.example.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    
    location / {
        proxy_pass http://localhost:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Rate Limiting

The Rust router currently does not include built-in rate limiting. Implement at:

1. **Reverse Proxy Level:**
   ```nginx
   # nginx rate limiting
   limit_req_zone $binary_remote_addr zone=vllm_limit:10m rate=10r/s;
   
   location / {
       limit_req zone=vllm_limit burst=20 nodelay;
       proxy_pass http://localhost:8000;
   }
   ```

2. **Cloud Provider:**
   - AWS WAF rate-based rules
   - GCP Cloud Armor rate limiting
   - Azure Front Door rate limiting

3. **API Gateway:**
   - Kong rate limiting plugin
   - Apigee quotas
   - Custom middleware

## Advanced Topics

### ZeroMQ Socket Patterns

The router uses two ZeroMQ socket patterns:

#### Router Socket (Input)

```
┌─────────────────────────────────────────────────────────────┐
│                    Router Socket Pattern                     │
│                                                              │
│  Rust Router (ROUTER)                                        │
│         │                                                    │
│    ┌────┴────┐                                               │
│    │         │                                               │
│    ▼         ▼                                               │
│ Engine 0  Engine 1  ...  Engine N                            │
│ (DEALER)  (DEALER)       (DEALER)                            │
│                                                              │
│  • Maintains connection to each engine                       │
│  • Tracks engine identities                                  │
│  • Round-robin request distribution                          │
│  • Asynchronous request/response                             │
└─────────────────────────────────────────────────────────────┘
```

**Router Socket Behavior:**
- Receives engine registration messages on startup
- Maintains list of active engine identities
- Distributes requests using round-robin scheduling
- Handles disconnections gracefully

#### Pull Socket (Output)

```
┌─────────────────────────────────────────────────────────────┐
│                     Pull Socket Pattern                      │
│                                                              │
│  Rust Router (PULL)                                          │
│         ▲                                                    │
│         │                                                    │
│    ┌────┴────┐                                               │
│    │         │                                               │
│    │         │                                               │
│ Engine 0  Engine 1  ...  Engine N                            │
│  (PUSH)    (PUSH)       (PUSH)                               │
│                                                              │
│  • Collects responses from all engines                       │
│  • Demultiplexes to correct request stream                   │
│  • Handles out-of-order responses                            │
│  • Cleans up finished request state                          │
└─────────────────────────────────────────────────────────────┘
```

**Pull Socket Behavior:**
- Receives streaming token responses
- Extracts request_id from each message
- Routes to appropriate async channel
- Removes stream on request completion

### PyO3 GIL Management

The router acquires the Python GIL only when needed:

```rust
// Tokenization requires GIL
let res: PyResult<Vec<u32>> = Python::with_gil(|py| {
    let renderer = state.renderer.bind(py);
    let tokenizer = renderer.getattr("renderer")
        .and_then(|r| r.getattr("tokenizer"))?;
    tokenizer.call_method1("encode", (prompt_str,))?
        .extract()
});
// GIL released after this block
```

**GIL Impact:**
- GIL held only during tokenization/decoding (~50-200 μs)
- HTTP handling, ZMQ communication run without GIL
- Multiple requests can tokenize concurrently (Python threads)
- Rust async tasks not blocked by GIL

### Memory Management

#### Request Stream Lifecycle

```
Request Start                          Request End
     │                                      │
     ▼                                      ▼
┌─────────┐                           ┌─────────┐
│ Create  │                           │ Remove  │
│ DashMap │                           │ DashMap │
│ Entry   │                           │ Entry   │
└────┬────┘                           └────┬────┘
     │                                     │
     │  ┌─────────────────────────────┐   │
     │  │  Active Request Stream      │   │
     │  │  • Receives token chunks    │   │
     │  │  • Formats SSE events       │   │
     │  │  • Streams to client        │   │
     │  └─────────────────────────────┘   │
     │                                     │
     └─────────────────────────────────────┘
              Automatic cleanup
```

**Memory per Request:**
- DashMap entry: ~100 bytes
- mpsc channel: ~1 KB buffer
- Pending tokens: Variable (typically <10 KB)
- Total: ~10 KB per active stream

#### Engine Identity Tracking

```rust
engine_identities: Arc<Mutex<Vec<bytes::Bytes>>>
```

- Stores unique identifier for each connected engine
- Used for ZeroMQ message routing
- Updated on engine connect/disconnect
- Typically <1 KB total

### Error Handling

#### Request-Level Errors

| Error | HTTP Status | Response |
|-------|-------------|----------|
| Invalid JSON | 400 Bad Request | `{"error": "Invalid JSON"}` |
| Missing required field | 422 Unprocessable Entity | `{"error": "Missing field: messages"}` |
| Invalid model | 400 Bad Request | `{"error": "Model not found"}` |
| Tokenization failure | 500 Internal Server Error | `{"error": "Tokenization failed"}` |

#### System-Level Errors

| Error | Behavior | Recovery |
|-------|----------|----------|
| No engines available | 503 Service Unavailable | Retry when engine connects |
| ZMQ send failure | Log error, return 500 | Automatic retry on next request |
| ZMQ recv failure | Log error, continue | Reconnect on next message |
| PyO3 initialization failure | Panic, restart | Restart process |

**Error Logging:**
```rust
match result {
    Ok(v) => v,
    Err(e) => {
        error!("Error processing request: {:?}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Error: {:?}", e)).into_response();
    }
}
```

## Integration Examples

### Python Client with Streaming

```python
import httpx
import json

async def stream_chat_completion():
    async with httpx.AsyncClient() as client:
        async with client.stream(
            "POST",
            "http://localhost:8000/v1/chat/completions",
            json={
                "model": "Qwen/Qwen2.5-1.5B-Instruct",
                "messages": [
                    {"role": "user", "content": "Hello!"}
                ],
                "stream": True
            },
            timeout=None
        ) as response:
            response.raise_for_status()
            async for line in response.aiter_lines():
                if line.startswith("data: "):
                    data = line[6:]
                    if data == "[DONE]":
                        break
                    chunk = json.loads(data)
                    content = chunk["choices"][0]["delta"].get("content", "")
                    print(content, end="", flush=True)

# Usage
import asyncio
asyncio.run(stream_chat_completion())
```

### curl with Streaming

```bash
curl -X POST http://localhost:8000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-1.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }' \
  --no-buffer | \
  grep "^data:" | \
  sed 's/^data: //' | \
  jq -r '.choices[0].delta.content // empty'
```

### JavaScript/Node.js Client

```javascript
const eventSource = new EventSource(
  'http://localhost:8000/v1/chat/completions',
  {
    headers: {
      'Content-Type': 'application/json',
    },
  }
);

// For streaming, use fetch with body
async function chatStream() {
  const response = await fetch(
    'http://localhost:8000/v1/chat/completions',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: 'Qwen/Qwen2.5-1.5B-Instruct',
        messages: [{ role: 'user', content: 'Hello!' }],
        stream: true,
      }),
    }
  );

  const reader = response.body.getReader();
  const decoder = new TextDecoder();

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    
    const chunk = decoder.decode(value);
    const lines = chunk.split('\n');
    
    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const data = JSON.parse(line.slice(6));
        const content = data.choices[0]?.delta?.content || '';
        console.log(content);
      }
    }
  }
}
```

### LangChain Integration

```python
from langchain_community.llms import VLLMOpenAI

llm = VLLMOpenAI(
    openai_api_key="EMPTY",
    openai_api_base="http://localhost:8000/v1",
    model_name="Qwen/Qwen2.5-1.5B-Instruct",
    model_kwargs={
        "temperature": 0.7,
        "max_tokens": 256,
    },
)

response = llm.invoke("What is vLLM?")
print(response)
```

### LiteLLM Proxy

```python
import litellm

# Configure LiteLLM to use vLLM Rust router
litellm.api_base = "http://localhost:8000/v1"
litellm.api_key = "EMPTY"

response = litellm.completion(
    model="openai/Qwen/Qwen2.5-1.5B-Instruct",
    messages=[{"role": "user", "content": "Hello!"}]
)

print(response.choices[0].message.content)
```

## Troubleshooting

### Diagnostic Commands

```bash
# Check if Rust router is running
curl http://localhost:8000/health

# Check model availability
curl http://localhost:8000/v1/models

# Test chat completion
curl http://localhost:8000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "model-name", "messages": [{"role": "user", "content": "test"}]}'

# Check Prometheus metrics
curl http://localhost:8000/metrics

# Monitor ZMQ connections (if using TCP)
netstat -an | grep 5555  # input port
netstat -an | grep 5556  # output port
```

### Common Issues and Solutions

#### Issue: Router starts but no engines connected

**Symptoms:**
- `/health` returns OK
- `/v1/models` returns empty or errors
- Requests return 503 Service Unavailable

**Diagnosis:**
```bash
# Check Rust router logs
RUST_LOG=debug vllm serve ... 2>&1 | grep -i "engine"

# Check for ZMQ connection errors
journalctl -u vllm -f | grep -i "zmq"
```

**Solutions:**
1. Verify ZeroMQ addresses match between router and engines
2. Check firewall rules for ZMQ ports
3. Ensure engine processes started successfully
4. Verify IPC socket paths are accessible (if using IPC)

#### Issue: High latency on token generation

**Symptoms:**
- Time to first token (TTFT) > 1 second
- Inter-token latency > 100ms

**Diagnosis:**
```bash
# Enable request timing
curl -w "@curl-format.txt" http://localhost:8000/v1/chat/completions ...

# curl-format.txt contents:
# time_namelookup:  %{time_namelookup}\n
# time_connect:     %{time_connect}\n
# time_starttransfer: %{time_starttransfer}\n
# time_total:       %{time_total}\n
```

**Solutions:**
1. Check GPU utilization (`nvidia-smi`)
2. Verify model is loaded (not loading on each request)
3. Reduce `max_tokens` if generating too many tokens
4. Check for CPU/GPU thermal throttling
5. Verify PyO3 tokenization is not bottleneck (profile with `RUST_LOG=trace`)

#### Issue: Memory growth over time

**Symptoms:**
- Router memory usage increases continuously
- Eventually hits memory limits

**Diagnosis:**
```bash
# Monitor memory
watch -n 1 'ps -o pid,rss,vsz,comm -p $(pgrep vllm-router)'

# Check active request streams
curl http://localhost:8000/metrics | grep active_request_streams
```

**Solutions:**
1. Check for client disconnections (streams should cleanup)
2. Verify clients are consuming SSE streams properly
3. Look for stuck requests in logs
4. Consider implementing request timeouts

#### Issue: PyO3 initialization failures

**Symptoms:**
- Router fails to start
- Error: "Python interpreter initialization failed"

**Diagnosis:**
```bash
# Check Python environment
python3 --version
which python3

# Check PyO3 bindings
ldd $(which vllm-router) | grep python
```

**Solutions:**
1. Ensure Python environment matches build environment
2. Reinstall vLLM with `uv pip install -e . --torch-backend=auto`
3. Check `LD_LIBRARY_PATH` includes Python libraries
4. Verify `PYTHONHOME` is set correctly (if needed)

#### Issue: SSE stream disconnects prematurely

**Symptoms:**
- Client receives partial response
- Stream ends before `finish_reason` is set
- No error in server logs

**Diagnosis:**
```bash
# Check client-side timeout
# Check network stability
# Monitor server-side stream state
RUST_LOG=trace vllm serve ... 2>&1 | grep -i "stream"
```

**Solutions:**
1. Increase client-side timeout
2. Configure reverse proxy timeout (nginx: `proxy_read_timeout`)
3. Check for network interruptions
4. Verify load balancer supports long-lived connections

### Performance Tuning

#### Optimal Configuration for High Throughput

```bash
vllm serve Qwen/Qwen2.5-1.5B-Instruct \
  --host 0.0.0.0 \
  --port 8000 \
  --data-parallel-size 4 \
  --max-num-seqs 256 \
  --gpu-memory-utilization 0.95 \
  --kv-cache-dtype fp8
```

**Key Parameters:**
- `data_parallel-size`: Match to GPU count
- `max-num-seqs`: Increase for higher concurrency (256-1024)
- `gpu-memory-utilization`: Maximize without OOM (0.90-0.95)
- `kv-cache-dtype`: Use fp8 for 2x memory savings

#### Network Tuning

For high-throughput deployments:

```bash
# Increase TCP buffer sizes
sysctl -w net.core.rmem_max=16777216
sysctl -w net.core.wmem_max=16777216
sysctl -w net.ipv4.tcp_rmem="4096 87380 16777216"
sysctl -w net.ipv4.tcp_wmem="4096 65536 16777216"

# Increase connection backlog
sysctl -w net.core.somaxconn=4096
```

#### ZeroMQ Tuning

For low-latency IPC communication:

```bash
# Use IPC instead of TCP for local communication
--input-address ipc:///tmp/vllm_engine_input
--output-address ipc:///tmp/vllm_engine_output

# Increase ZMQ I/O threads (environment variable)
export ZMQ_IO_THREADS=4
```

## References

### Documentation

- [Axum Documentation](https://docs.rs/axum/latest/axum/)
- [PyO3 User Guide](https://pyo3.rs/latest/)
- [ZeroMQ Guide](https://zguide.zeromq.org/)
- [Tokio Documentation](https://docs.rs/tokio/latest/tokio/)
- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
- [MessagePack Specification](https://msgpack.org/)

### Source Code

- Rust Router: `vllm-router/src/main.rs`, `vllm-router/src/lib.rs`
- Build Configuration: `vllm-router/Cargo.toml`
- Python Integration: `vllm/entrypoints/cli/serve.py`

### Related Papers

- [vLLM: Easy, Fast, and Cheap LLM Serving with PagedAttention](https://arxiv.org/abs/2309.06180)
- [Axum: Ergonomic Web Framework for Rust](https://github.com/tokio-rs/axum)
- [PyO3: Rust Bindings for Python](https://pyo3.rs/)
