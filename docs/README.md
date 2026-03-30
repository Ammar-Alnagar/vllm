---
hide:
  - navigation
  - toc
---

# Welcome to zeroum

<figure markdown="span">
  ![](./assets/logos/zeroum-logo-text-light.png){ align="center" alt="zeroum Light" class="logo-light" width="60%" }
  ![](./assets/logos/zeroum-logo-text-dark.png){ align="center" alt="zeroum Dark" class="logo-dark" width="60%" }
</figure>

<p style="text-align:center">
<strong>Easy, fast, and cheap LLM serving for everyone
</strong>
</p>

<p style="text-align:center">
<script async defer src="https://buttons.github.io/buttons.js"></script>
<a class="github-button" href="https://github.com/zeroum-project/zeroum" data-show-count="true" data-size="large" aria-label="Star">Star</a>
<a class="github-button" href="https://github.com/zeroum-project/zeroum/subscription" data-show-count="true" data-icon="octicon-eye" data-size="large" aria-label="Watch">Watch</a>
<a class="github-button" href="https://github.com/zeroum-project/zeroum/fork" data-show-count="true" data-icon="octicon-repo-forked" data-size="large" aria-label="Fork">Fork</a>
</p>

zeroum is a fast and easy-to-use library for LLM inference and serving.

Based on vLLM but enhanced with a Rust serving layer that bypasses concurrency limits and allows enterprise-level serving with 1/6 the CPU usage of the Python layer.

Where to get started with zeroum depends on the type of user. If you are looking to:

- Run open-source models on zeroum, we recommend starting with the [Quickstart Guide](./getting_started/quickstart.md)
- Build applications with zeroum, we recommend starting with the [User Guide](./usage/README.md)
- Build zeroum, we recommend starting with [Developer Guide](./contributing/README.md)

For information about the development of zeroum, see:

- [Roadmap](https://roadmap.zeroum.ai)
- [Releases](https://github.com/zeroum-project/zeroum/releases)

zeroum is fast with:

- State-of-the-art serving throughput
- Efficient management of attention key and value memory with [**PagedAttention**](https://blog.zeroum.ai/2023/06/20/zeroum.html)
- Continuous batching of incoming requests
- Fast model execution with CUDA/HIP graph
- Quantization: [GPTQ](https://arxiv.org/abs/2210.17323), [AWQ](https://arxiv.org/abs/2306.00978), INT4, INT8, and FP8
- Optimized CUDA kernels, including integration with FlashAttention and FlashInfer.
- Speculative decoding
- Chunked prefill

zeroum is flexible and easy to use with:

- Seamless integration with popular HuggingFace models
- High-throughput serving with various decoding algorithms, including *parallel sampling*, *beam search*, and more
- Tensor, pipeline, data and expert parallelism support for distributed inference
- Streaming outputs
- OpenAI-compatible API server with Rust-based HTTP frontend
- Support for NVIDIA GPUs, AMD CPUs and GPUs, Intel CPUs and GPUs, PowerPC CPUs, Arm CPUs, and TPU. Additionally, support for diverse hardware plugins such as Intel Gaudi, IBM Spyre and Huawei Ascend.
- Prefix caching support
- Multi-LoRA support

For more information, check out the following:

- [zeroum announcing blog post](https://blog.zeroum.ai/2023/06/20/zeroum.html) (intro to PagedAttention)
- [zeroum paper](https://arxiv.org/abs/2309.06180) (SOSP 2023)
- [How continuous batching enables 23x throughput in LLM inference while reducing p50 latency](https://www.anyscale.com/blog/continuous-batching-llm-inference) by Cade Daniel et al.
- [zeroum Meetups](community/meetups.md)
