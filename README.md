# Model2Vec Rust API Server

<p align="center">
  <img src="https://img.shields.io/crates/v/model2vec-rs.svg" alt="Crates.io">
  <img src="https://img.shields.io/badge/Rust-1.83+-orange.svg" alt="Rust Version">
  <img src="https://img.shields.io/badge/Docker-Ready-blue.svg" alt="Docker">
</p>

A **high-performance**, OpenAI-compatible embedding API server built with **Rust** and **[model2vec-rs](https://github.com/MinishLab/model2vec-rs)**. This server loads models directly from HuggingFace Hub using the official Rust crate - **no Python dependencies required**.

## Why Rust?

| Implementation | Throughput | Memory Usage |
|----------------|------------|--------------|
| **Rust (model2vec-rs)** | **8,000 samples/sec** | ~50-100 MB |
| Python (model2vec) | 4,650 samples/sec | ~1.2 GB |

The Rust implementation is **1.7x faster** with **~80% less memory**.

## Quick Start

### Docker (Recommended)

```bash
# Build and run
docker compose up -d

# Test the API
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"input": "hello world", "model": "minishlab/potion-base-8M"}'
```

### Local Development

```bash
# Install Rust and model2vec-rs CLI
cargo install model2vec-rs

# Clone and build
git clone https://github.com/tan-yong-sheng/model2vec-rs-api-server.git
cd model2vec-rs-api-server
cargo build --release

# Run
./target/release/model2vec-api
```

## Available Models

All models are loaded directly from [HuggingFace Hub](https://huggingface.co/collections/minishlab/model2vec-base-models-66fd9dd9b7c3b3c0f25ca90e) using the Rust `model2vec-rs` crate.

| Model | Language | Parameters | Dimensions | Size | Best For |
|-------|----------|------------|------------|------|----------|
| [potion-base-2M](https://huggingface.co/minishlab/potion-base-2M) | English | 1.8M | 384 | ~2 MB | Resource-constrained |
| [potion-base-4M](https://huggingface.co/minishlab/potion-base-4M) | English | 3.7M | 384 | ~4 MB | Balanced |
| [potion-base-8M](https://huggingface.co/minishlab/potion-base-8M) | English | 7.5M | 768 | ~8 MB | **Default** |
| [potion-base-32M](https://huggingface.co/minishlab/potion-base-32M) | English | 32.3M | 1024 | ~32 MB | High quality |
| [potion-retrieval-32M](https://huggingface.co/minishlab/potion-retrieval-32M) | English | 32.3M | 1024 | ~32 MB | Search/Retrieval |
| [potion-multilingual-128M](https://huggingface.co/minishlab/potion-multilingual-128M) | Multilingual | 128M | 768 | ~128 MB | Multi-language |

## API Endpoints

### Generate Embeddings

```bash
# Single string
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"input": "hello world", "model": "minishlab/potion-base-8M"}'

# Array of strings
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"input": ["hello", "world"], "model": "minishlab/potion-base-8M"}'

# With base64 encoding
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"input": "hello", "model": "minishlab/potion-base-8M", "encoding_format": "base64"}'
```

### List Models

```bash
curl http://localhost:8080/v1/models
```

### Health Checks

```bash
# Liveness
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/.well-known/live
# Returns: 204

# Readiness
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/.well-known/ready
# Returns: 204
```

## Configuration

Create a `.env` file:

```bash
MODEL_NAME=minishlab/potion-base-8M
ALIAS_MODEL_NAME=my-model
PORT=8080
AUTHENTICATION_ALLOWED_TOKENS=token1,token2
```

| Variable | Default | Description |
|----------|---------|-------------|
| `MODEL_NAME` | minishlab/potion-base-8M | HuggingFace model ID |
| `ALIAS_MODEL_NAME` | - | Optional model alias |
| `PORT` | 8080 | Server port |
| `AUTHENTICATION_ALLOWED_TOKENS` | - | Comma-separated tokens |

## Architecture

```
model2vec-rs-api-server/
├── src/
│   ├── main.rs              # Entry point
│   ├── config/mod.rs        # Configuration
│   ├── vectorizer/mod.rs    # Model loading + Moka cache
│   └── app/
│       ├── mod.rs           # App state & router
│       ├── routes.rs        # HTTP handlers
│       ├── models.rs        # Request/Response types
│       └── auth.rs          # Bearer token auth
├── Dockerfile               # Multi-stage Docker build
├── docker-compose.yml       # Container orchestration
└── DEPLOY.md               # Full deployment guide
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Web Framework | Axum 0.8 |
| Async Runtime | Tokio 1.x |
| Model Inference | model2vec-rs 0.1.4 |
| Caching | Moka (TTLCache) |
| Config | dotenvy |
| Serialization | serde + serde_json |

## Performance

- **Throughput**: ~8,000 samples/second (single-threaded CPU)
- **Memory Usage**: ~50-100 MB

## Resources

- [GitHub Repository](https://github.com/tan-yong-sheng/model2vec-rs-api-server)
- [model2vec-rs Documentation](https://github.com/MinishLab/model2vec-rs)
- [Model2Vec Models on HuggingFace](https://huggingface.co/collections/minishlab/model2vec-base-models-66fd9dd9b7c3b3c0f25ca90e)
- [OpenAI Embeddings API](https://platform.openai.com/docs/guides/embeddings)

## License

MIT License - see [LICENSE](LICENSE) for details.

## Citation

Credit to: [https://github.com/MinishLab/model2vec-rs](https://github.com/MinishLab/model2vec-rs)

If you use Model2Vec in your research, please cite:

```bibtex
@article{minishlab2024model2vec,
  author = {Tulkens, Stephan and {van Dongen}, Thomas},
  title = {Model2Vec: Fast State-of-the-Art Static Embeddings},
  year = {2024},
  url = {https://github.com/MinishLab/model2vec}
}
```
