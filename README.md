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

<p align="center">
| <a href="https://docs.zeroum.ai"><b>Documentation</b></a> | <a href="https://blog.zeroum.ai/"><b>Blog</b></a> | <a href="https://arxiv.org/abs/2309.06180"><b>Paper</b></a> | <a href="https://x.com/zeroum_project"><b>Twitter/X</b></a> | <a href="https://discuss.zeroum.ai"><b>User Forum</b></a> | <a href="https://slack.zeroum.ai"><b>Developer Slack</b></a> |
</p>

---

## About

zeroum is a fast and easy-to-use library for LLM inference and serving.

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

## Getting Started

Install zeroum with `pip` or [from source](https://docs.zeroum.ai/en/latest/getting_started/installation/gpu/index.html#build-wheel-from-source):

```bash
pip install zeroum
```

Visit our [documentation](https://docs.zeroum.ai/en/latest/) to learn more.

- [Installation](https://docs.zeroum.ai/en/latest/getting_started/installation.html)
- [Quickstart](https://docs.zeroum.ai/en/latest/getting_started/quickstart.html)
- [List of Supported Models](https://docs.zeroum.ai/en/latest/models/supported_models.html)

## Contributing

We welcome and value any contributions and collaborations.
Please check out [Contributing to ZeroUm](https://docs.zeroum.ai/en/latest/contributing/index.html) for how to get involved.

## Citation

If you use ZeroUm for your research, please cite our [paper](https://arxiv.org/abs/2309.06180):

```bibtex
@inproceedings{kwon2023efficient,
  title={Efficient Memory Management for Large Language Model Serving with PagedAttention},
  author={Woosuk Kwon and Zhuohan Li and Siyuan Zhuang and Ying Sheng and Lianmin Zheng and Cody Hao Yu and Joseph E. Gonzalez and Hao Zhang and Ion Stoica},
  booktitle={Proceedings of the ACM SIGOPS 29th Symposium on Operating Systems Principles},
  year={2023}
}
```

## Contact Us

<!-- --8<-- [start:contact-us] -->
- For technical questions and feature requests, please use GitHub [Issues](https://github.com/zeroum-project/zeroum/issues)
- For discussing with fellow users, please use the [ZeroUm Forum](https://discuss.zeroum.ai)
- For coordinating contributions and development, please use [Slack](https://slack.zeroum.ai)
- For security disclosures, please use GitHub's [Security Advisories](https://github.com/zeroum-project/zeroum/security/advisories) feature
- For collaborations and partnerships, please contact us at [collaboration@zeroum.ai](mailto:collaboration@zeroum.ai)
<!-- --8<-- [end:contact-us] -->

## Media Kit

- If you wish to use ZeroUm's logo, please refer to [our media kit repo](https://github.com/zeroum-project/media-kit)
