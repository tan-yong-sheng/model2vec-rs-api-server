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

---

# Docker Hub Publishing via GitHub Actions

## Overview

Set up automated Docker image builds and publishing to Docker Hub using GitHub Actions workflows.

## Prerequisites

1. **Docker Hub Account**: Create account at https://hub.docker.com
2. **Docker Hub Access Token**: Generate at https://hub.docker.com/settings/security
3. **GitHub Repository**: Push code to GitHub

## Required Secrets (GitHub Repository Settings)

| Secret Name | Value | Description |
|-------------|-------|-------------|
| `DOCKERHUB_USERNAME` | Your Docker Hub username | For image tagging |
| `DOCKERHUB_TOKEN` | Docker Hub access token | For authentication |

## Setting Up GitHub Secrets

1. Go to repository Settings → Secrets and variables → Actions
2. Add `DOCKERHUB_USERNAME` (your Docker Hub username)
3. Add `DOCKERHUB_TOKEN` (access token from Docker Hub)

## Workflow File

Create `.github/workflows/docker-publish.yml`:

```yaml
name: Build and Push to Docker Hub

on:
  push:
    branches: [main]
    tags: ['v*']
  pull_request:
    branches: [main]

env:
  REGISTRY: docker.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  build:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write

    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to Docker Hub
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ secrets.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ secrets.DOCKERHUB_USERNAME }}/${{ env.IMAGE_NAME }}
          tags: |
            type=ref,event=branch
            type=ref,event=pr
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=sha

      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          push: ${{ github.event_name != 'pull_request' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Show image digest
        run: echo ${{ steps.build.outputs.digest }}
```

## Image Naming Convention

| Trigger | Image Tag | Example |
|---------|-----------|---------|
| Push to main | `latest`, `main-{sha}` | `docker.io/user/model2vec-rs-api-server:latest` |
| Tag v1.0.0 | `1.0.0`, `1.0`, `1` | `docker.io/user/model2vec-rs-api-server:1.0.0` |
| PR | `{pr-number}-{sha}` | `docker.io/user/model2vec-rs-api-server:42-abc123` |

## Pull Commands

After successful publish:

```bash
# Latest version
docker pull docker.io/{DOCKERHUB_USERNAME}/model2vec-rs-api-server:latest

# Specific version
docker pull docker.io/{DOCKERHUB_USERNAME}/model2vec-rs-api-server:1.0.0
```

## Running the Image

```bash
# Basic
docker run -p 8080:8080 docker.io/{DOCKERHUB_USERNAME}/model2vec-rs-api-server:latest

# With custom model
docker run -p 8080:8080 \
  -e MODEL_NAME=minishlab/potion-base-8M \
  docker.io/{DOCKERHUB_USERNAME}/model2vec-rs-api-server:latest

# With authentication
docker run -p 8080:8080 \
  -e AUTHENTICATION_ALLOWED_TOKENS=my-secret-token \
  docker.io/{DOCKERHUB_USERNAME}/model2vec-rs-api-server:latest
```

## Implementation Tasks

### Phase 1: Docker Hub Setup

1. Create Docker Hub account (if not exists)
2. Create repository on Docker Hub: `model2vec-rs-api-server`
3. Generate Docker Hub access token
4. Add secrets to GitHub repository

### Phase 2: GitHub Actions Workflow

1. Create `.github/workflows/docker-publish.yml`
2. Test workflow on push to main
3. Test workflow on tag push (v*)
4. Verify image published to Docker Hub

### Phase 3: Testing & Verification

1. Pull published image
2. Run container locally
3. Verify API endpoints work
4. Test with curl requests

### Phase 4: Security Scanning (Optional)

Add Trivy security scanning:

```yaml
- name: Run Trivy vulnerability scanner
  uses: aquasecurity/trivy-action@master
  with:
    image-ref: ${{ env.REGISTRY }}/${{ secrets.DOCKERHUB_USERNAME }}/${{ env.IMAGE_NAME }}:${{ steps.meta.outputs.version }}
    format: 'sarif'
    output: 'trivy-results.sarif'

- name: Upload Trivy results
  uses: github/codeql-action/upload-sarif@v2
  with:
    sarif_file: 'trivy-results.sarif'
```

## Multi-Architecture Builds (Optional)

For ARM64/AMD64 support:

```yaml
- name: Set up QEMU
  uses: docker/setup-qemu-action@v3

- name: Set up Docker Buildx
  uses: docker/setup-buildx-action@v3

- name: Build and push
  uses: docker/build-push-action@v5
  with:
    context: .
    push: ${{ github.event_name != 'pull_request' }}
    platforms: linux/amd64,linux/arm64
    tags: ${{ steps.meta.outputs.tags }}
    labels: ${{ steps.meta.outputs.labels }}
```

## Resources

- **GitHub Actions Documentation**: https://docs.github.com/en/actions
- **Docker Buildx**: https://docs.docker.com/build/buildx/
- **Docker Hub API**: https://docs.docker.com/docker-hub/api/latest/
- **AGENTS.md**: See CI/CD skills section for workflow patterns
- **TODO.md**: Track implementation progress
