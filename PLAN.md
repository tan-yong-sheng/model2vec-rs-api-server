# PLAN.md - Model2Vec Rust Migration Plan

## Overview

Convert the Python model2vec embedding API server to Rust using the official `model2vec-rs` crate for significant performance and memory improvements.

## Tech Stack

- **Web Framework**: Axum 0.8 (Async, based on Tokio)
- **Model Inference**: model2vec-rs 0.3 (Official Rust implementation)
- **Caching**: Moka with future support (TTLCache equivalent)
- **Configuration**: dotenvy (Environment variables)
- **Serialization**: serde + serde_json
- **Validation**: validator crate
- **Authentication**: tower-http (Bearer token)
- **Container**: Multi-stage Docker build

## Project Structure

```
model2vec-rs-api-server/
├── Cargo.toml              # Rust dependencies
├── src/
│   ├── main.rs             # Entry point with Tokio runtime
│   ├── config/
│   │   └── mod.rs          # Config struct, env var parsing
│   ├── vectorizer/
│   │   └── mod.rs          # StaticModel wrapper + Moka cache
│   ├── app/
│   │   ├── mod.rs          # AppState, router creation
│   │   ├── routes.rs       # All HTTP handlers
│   │   ├── models.rs       # Request/Response Pydantic equivalents
│   │   └── auth.rs         # Auth middleware
│   └── lib.rs              # Library exports
├── Dockerfile              # Multi-stage: build → runtime
├── docker-compose.yml      # Service definition
├── .env.example            # Environment template
└── models/                 # Mounted model directory
```

## Implementation Steps

### Phase 1: Project Setup

1. Create Cargo.toml with all dependencies
2. Set up project directory structure
3. Create .env.example with required env vars

### Phase 2: Configuration Layer

1. Implement `Config` struct in `src/config/mod.rs`
2. Parse `MODEL_NAME`, `ALIAS_MODEL_NAME`, `AUTHENTICATION_ALLOWED_TOKENS`, `PORT`
3. Provide defaults: MODEL_NAME="minishlab/potion-base-8M", PORT=8080

### Phase 3: Vectorizer Module

1. Load model from `./models` directory using `StaticModel::from_directory()`
2. Implement `encode()` method handling single string or Vec<String>
3. Add Moka TTLCache with maxsize=1024, ttl=600 seconds
4. Cache key: (input_text, config_options)

### Phase 4: HTTP Handlers

1. **Health endpoints**:
   - `/.well-known/live` → 204 No Content
   - `/.well-known/ready` → 204 No Content

2. **Meta endpoint**:
   - `GET /meta` → Return model config (JSON from config.json)

3. **Models endpoint**:
   - `GET /v1/models` → Return list with id, object, created, owned_by

4. **Embeddings endpoint**:
   - `POST /v1/embeddings` → Accept VectorInput, return EmbeddingResponse
   - Support `encoding_format`: "float" or "base64"
   - Support `dimensions` parameter for truncation
   - Support both string and array input

### Phase 5: Authentication

1. Extract `Authorization: Bearer <token>` header
2. Validate against `AUTHENTICATION_ALLOWED_TOKENS`
3. Return 401 Unauthorized if invalid/missing (when auth enabled)

### Phase 6: Docker Containerization

1. Multi-stage Dockerfile:
   - Build stage: rust:1.75-alpine with build dependencies
   - Runtime stage: alpine:3.19 with binary and models
2. Strip binary for smaller size
3. docker-compose.yml with proper environment variables

### Phase 7: Testing

1. Health check: `curl http://localhost:8080/.well-known/ready`
2. Embeddings with string: `curl -X POST http://localhost:8080/v1/embeddings -H "Content-Type: application/json" -d '{"input": "hello world", "model": "minishlab/potion-base-8M"}'`
3. Embeddings with array: `curl -X POST http://localhost:8080/v1/embeddings -H "Content-Type: application/json" -d '{"input": ["hello", "world"], "model": "minishlab/potion-base-8M"}'`
4. Base64 encoding: Add `"encoding_format": "base64"` to request
5. Auth protection: Test with invalid Bearer token

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `MODEL_NAME` | No | minishlab/potion-base-8M | HuggingFace model ID or path |
| `ALIAS_MODEL_NAME` | No | None | Optional model alias |
| `AUTHENTICATION_ALLOWED_TOKENS` | No | (none) | Comma-separated bearer tokens |
| `PORT` | No | 8080 | HTTP server port |

## Expected Memory Savings

| Component | Python | Rust | Reduction |
|-----------|--------|------|-----------|
| Interpreter/Binary | 100-150 MB | 5-15 MB | ~90% |
| Web Framework | 50-100 MB | 10-20 MB | ~80% |
| Model Loading | 800+ MB | 30-60 MB | ~95% |
| Caching | 50-100 MB | 50-100 MB | ~0% |
| **Total** | **~1.2 GB** | **~150-250 MB** | **~80%** |

## Performance Target

- **Throughput**: 8000+ samples/second (vs 4650 Python)
- **Latency**: <10ms per embedding request
- **Startup Time**: <5 seconds
