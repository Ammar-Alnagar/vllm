<!-- markdownlint-disable MD001 MD041 -->
<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/zeroum-project/zeroum/main/docs/assets/logos/zeroum-logo-text-dark.png">
    <img alt="ZeroUm" src="https://raw.githubusercontent.com/zeroum-project/zeroum/main/docs/assets/logos/zeroum-logo-text-light.png" width=55%>
  </picture>
</p>

<h3 align="center">
Easy, fast, and cheap LLM serving for everyone
</h3>


---

## About

zeroum is a fast and easy-to-use library for LLM inference and serving(based on VLLM).

zeroum is enhanced with a Rust serving layer that bypasses concurrency limits and allows enterprise-level serving with 1/6 the CPU usage of the Python layer.

zeroum is fast with:

- State-of-the-art serving throughput
- **High-performance Rust Axum HTTP server** for low-latency request handling
- Efficient management of attention key and value memory with [**PagedAttention**](https://blog.zeroum.ai/2023/06/20/zeroum.html)
- Continuous batching of incoming requests
- Fast model execution with CUDA/HIP graph
- Quantizations: [GPTQ](https://arxiv.org/abs/2210.17323), [AWQ](https://arxiv.org/abs/2306.00978), [AutoRound](https://arxiv.org/abs/2309.05516), INT4, INT8, and FP8
- Optimized CUDA kernels, including integration with FlashAttention and FlashInfer
- Speculative decoding
- Chunked prefill

zeroum is flexible and easy to use with:

- Seamless integration with popular Hugging Face models
- High-throughput serving with various decoding algorithms, including *parallel sampling*, *beam search*, and more
- Tensor, pipeline, data and expert parallelism support for distributed inference
- Streaming outputs
- **OpenAI-compatible API server with Rust-based HTTP frontend**
- Support for NVIDIA GPUs, AMD CPUs and GPUs, Intel CPUs and GPUs, PowerPC CPUs, Arm CPUs, and TPU. Additionally, support for diverse hardware plugins such as Intel Gaudi, IBM Spyre and Huawei Ascend.
- Prefix caching support
- Multi-LoRA support

zeroum seamlessly supports most popular open-source models on HuggingFace, including:

- Transformer-like LLMs (e.g., Llama)
- Mixture-of-Expert LLMs (e.g., Mixtral, Deepseek-V2 and V3)
- Embedding Models (e.g., E5-Mistral)
- Multi-modal LLMs (e.g., LLaVA)

Find the full list of supported models [here](https://docs.zeroum.ai/en/latest/models/supported_models.html).

## Rust HTTP Server Architecture

zeroum uses a high-performance Rust-based HTTP server built with the [Axum](https://github.com/tokio-rs/axum) framework. This architecture provides:

- **Lower Latency**: Rust's zero-cost abstractions and async runtime reduce request handling overhead
- **Higher Throughput**: True parallelism without GIL limitations enables better concurrency
- **OpenAI Compatibility**: Full support for OpenAI's Chat Completions, Completions, and Embeddings APIs
- **Hybrid Design**: PyO3 integration calls Python for tokenization while Rust handles HTTP I/O
- **ZeroMQ Communication**: Efficient message passing between the Rust router and zeroum engine cores

```
┌──────────────┐      HTTP      ┌──────────────────┐    ZeroMQ    ┌─────────────────┐
│   Client     │ ◄────────────► │  Rust Axum       │ ◄──────────► │  zeroum Engine   │
│              │                │  HTTP Server     │              │  (Python)       │
└──────────────┘                │  • OpenAI API    │              │  • PagedAttention│
                                │  • Tokenization  │              │  • Model Exec   │
                                │    (PyO3)        │              │  • KV Cache     │
                                │  • Load Balancing│              │                 │
                                └──────────────────┘              └─────────────────┘
```

For detailed architecture information, see the [Rust Router Architecture](https://docs.zeroum.ai/en/latest/design/rust_router_architecture.html) documentation.
