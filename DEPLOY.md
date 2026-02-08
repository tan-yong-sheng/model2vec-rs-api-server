# Deployment Guide for Model2Vec Rust API Server

A high-performance, OpenAI-compatible embedding API server built with **Rust** and **model2vec-rs**. This server loads models directly from HuggingFace Hub using the official Rust crate - no Python dependencies required.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Local Deployment (Without Docker)](#local-deployment-without-docker)
- [Docker Deployment](#docker-deployment)
- [Available Models](#available-models)
- [API Endpoints](#api-endpoints)
- [Example Usage](#example-usage)
- [Configuration](#configuration)
- [Production Considerations](#production-considerations)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### For Local Deployment

- **Rust 1.83+** and Cargo
- **Git** (for HuggingFace model downloads)
- ~500MB disk space for model cache

### For Docker Deployment

- Docker Engine 20.10+
- Docker Compose V2
- ~1GB disk space

---

## Local Deployment (Without Docker)

### Step 1: Install Rust

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify Rust version
rustc --version  # Should be 1.83+
```

### Step 2: Install model2vec-rs CLI

Install the Rust CLI tool for model downloading:

```bash
cargo install model2vec-rs
```

This downloads and compiles the CLI in release mode for optimal performance.

### Step 3: Clone and Build the API Server

```bash
# Clone the repository
git clone https://github.com/tan-yong-sheng/model2vec-rs-api-server.git
cd model2vec-rs-api-server

# Build the project in release mode
cargo build --release
```

### Step 4: Configure Environment

```bash
# Copy environment template
cp .env.example .env

# Edit .env with your settings (see Configuration section below)
```

### Step 5: Run the Server

```bash
# Run the API server
./target/release/model2vec-api

# Or with custom port
PORT=8080 ./target/release/model2vec-api
```

The server will start on `http://0.0.0.0:8080`.

---

## Docker Deployment

### Step 1: Build the Image

The Docker image uses a multi-stage build that:
1. Compiles the Rust binary
2. Downloads model files from HuggingFace using git
3. Creates a minimal runtime image

```bash
# Build with default model (minishlab/potion-base-8M)
docker compose build

# Build with a specific model
MODEL_NAME=minishlab/potion-base-32M docker compose build
```

### Step 2: Run the Container

```bash
# Start the service in detached mode
docker compose up -d

# View logs
docker compose logs -f

# Stop the service
docker compose down
```

---

## Available Models

All models are loaded directly from [HuggingFace Hub](https://huggingface.co/collections/minishlab/model2vec-base-models-66fd9dd9b7c3b3c0f25ca90e) using the Rust `model2vec-rs` crate.

| Model ID | Language | Source Model | Parameters | Dimensions | Task | Size |
|----------|----------|--------------|------------|------------|------|------|
| [minishlab/potion-base-2M](https://huggingface.co/minishlab/potion-base-2M) | English | bge-base-en-v1.5 | 1.8M | 384 | General | ~2 MB |
| [minishlab/potion-base-4M](https://huggingface.co/minishlab/potion-base-4M) | English | bge-base-en-v1.5 | 3.7M | 384 | General | ~4 MB |
| [minishlab/potion-base-8M](https://huggingface.co/minishlab/potion-base-8M) | English | bge-base-en-v1.5 | 7.5M | 768 | General | ~8 MB |
| [minishlab/potion-base-32M](https://huggingface.co/minishlab/potion-base-32M) | English | bge-base-en-v1.5 | 32.3M | 1024 | General | ~32 MB |
| [minishlab/potion-retrieval-32M](https://huggingface.co/minishlab/potion-retrieval-32M) | English | bge-base-en-v1.5 | 32.3M | 1024 | Retrieval | ~32 MB |
| [minishlab/potion-multilingual-128M](https://huggingface.co/minishlab/potion-multilingual-128M) | Multilingual | bge-m3 | 128M | 768 | General | ~128 MB |

### Performance Comparison (Single-threaded CPU)

| Implementation | Throughput |
|----------------|------------|
| **Rust (model2vec-rs)** | **8,000 samples/sec** |
| Python (model2vec) | 4,650 samples/sec |

The Rust implementation is **1.7x faster**.

### Choosing a Model

- **potion-base-2M/4M**: Best for resource-constrained environments
- **potion-base-8M**: Balanced performance and quality (default)
- **potion-base-32M**: Highest quality for English
- **potion-retrieval-32M**: Optimized for retrieval/search tasks
- **potion-multilingual-128M**: Best for multilingual content

---

## API Endpoints

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/.well-known/live` | GET | No | Health check (returns 204) |
| `/.well-known/ready` | GET | No | Readiness check (returns 204) |
| `/meta` | GET | Optional | Model metadata |
| `/v1/models` | GET | Optional | List available models |
| `/v1/embeddings` | POST | Optional | Generate embeddings |

---

## Example Usage

### Generate Embedding (Single String)

```bash
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "input": "hello world",
    "model": "minishlab/potion-base-8M"
  }'
```

### Generate Embeddings (Array of Strings)

```bash
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "input": ["hello world", "how are you", "goodbye"],
    "model": "minishlab/potion-base-8M"
  }'
```

### With Base64 Encoding

```bash
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "input": "hello world",
    "model": "minishlab/potion-base-8M",
    "encoding_format": "base64"
  }'
```

### With Dimensions Truncation

```bash
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "input": "hello world",
    "model": "minishlab/potion-base-8M",
    "dimensions": 256
  }'
```

### With Authentication

```bash
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-token" \
  -d '{
    "input": "hello world",
    "model": "minishlab/potion-base-8M"
  }'
```

### List Models

```bash
curl http://localhost:8080/v1/models
```

### Health Check

```bash
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/.well-known/ready
# Returns: 204
```

---

## Configuration

Create a `.env` file in the project root:

```bash
# Model configuration (HuggingFace model ID or local path)
MODEL_NAME=minishlab/potion-base-8M

# Optional model alias for API responses
ALIAS_MODEL_NAME=my-embedding-model

# Server port
PORT=8080

# Lazy load model (optional) - set to true to delay model loading until first request
# Default: false (model loads at startup)
# Recommended: false for small models, true for large models (128M+) to reduce startup time
LAZY_LOAD_MODEL=false

# Optional: Authentication (comma-separated bearer tokens)
AUTHENTICATION_ALLOWED_TOKENS=token1,token2,token3
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `MODEL_NAME` | No | minishlab/potion-base-8M | HuggingFace model ID |
| `ALIAS_MODEL_NAME` | No | - | Optional model alias |
| `PORT` | No | 8080 | HTTP server port |
| `LAZY_LOAD_MODEL` | No | false | When `true`, delays model loading until first request. Useful for large models (128M+) to reduce startup time |
| `AUTHENTICATION_ALLOWED_TOKENS` | No | - | Comma-separated tokens |

---

## Production Considerations

### Resource Usage

| Metric | Value |
|--------|-------|
| RAM Usage | ~150-250 MB |
| Disk (model cache) | 2-128 MB (varies by model) |
| Throughput | ~8,000 samples/sec |
| Cold Start | ~3-5 minutes (model download) |

### Lazy Loading vs Eager Loading

The server supports two model loading strategies:

#### Eager Loading (Default: `LAZY_LOAD_MODEL=false`)
- Model loads during server startup
- First request is fast (already loaded)
- Startup takes longer (can be several minutes for large models)
- Recommended for small models (8M, 32M) and production environments

#### Lazy Loading (`LAZY_LOAD_MODEL=true`)
- Server starts immediately without loading the model
- Model loads on first embedding request
- First request is slower (includes model loading time)
- Recommended for large models (128M+) and development environments

**Note:** Health check endpoints (`/.well-known/live` and `/.well-known/ready`) return 204 immediately regardless of model loading state.

**Example with 128M multilingual model:**
```bash
# Eager loading: ~2-3 minutes startup, instant first request
LAZY_LOAD_MODEL=false ./target/release/model2vec-api

# Lazy loading: instant startup, ~2-3 minutes first request
LAZY_LOAD_MODEL=true ./target/release/model2vec-api
```

**When to use lazy loading:**
- Large models (128M+) with long load times
- Development/testing environments
- Kubernetes deployments with health check timeouts
- When you need the server to be "ready" quickly

**When to use eager loading:**
- Production environments requiring consistent performance
- Small to medium models (2M-32M)
- When first request latency is critical

### Security

1. **Authentication**: Set `AUTHENTICATION_ALLOWED_TOKENS` to enable Bearer token auth
2. **Network**: Bind to `0.0.0.0` only when behind a reverse proxy
3. **Secrets**: Use Docker secrets or external config management

### Scaling

- The service is stateless and can be scaled horizontally
- Use a load balancer with sticky sessions if needed
- Each instance downloads its own model from HuggingFace

### Monitoring

- Health checks: `/.well-known/ready`
- Logs: Check container logs or stdout
- Metrics: Integrate with Prometheus (custom implementation needed)

---

## Troubleshooting

### Model Download Fails

```bash
# Clear HuggingFace cache
rm -rf ~/.cache/hf/hub

# Verify git is installed
git --version
```

### Port Already in Use

```bash
# Change port in .env
PORT=8081

# Or kill the process using the port
lsof -i :8080
kill -9 <PID>
```

### Docker Build Fails

```bash
# Clear Docker build cache
docker system prune -a

# Try again with no cache
docker compose build --no-cache
```

### Out of Memory

Use a smaller model:

```bash
# Use potion-base-4M instead of potion-base-32M
MODEL_NAME=minishlab/potion-base-4M docker compose build --no-cache
```

---

## Sources

- [model2vec-rs GitHub Repository](https://github.com/MinishLab/model2vec-rs)
- [Model2Vec Models on HuggingFace](https://huggingface.co/collections/minishlab/model2vec-base-models-66fd9dd9b7c3b3c0f25ca90e)
- [Model2Vec: Fast State-of-the-Art Static Embeddings](https://github.com/MinishLab/model2vec)
