# AGENTS.md - Model2Vec Rust Migration

## Skills Required

| Skill | Purpose |
|-------|---------|
| `rust-pro` | Rust 1.75+ development, Tokio, Axum web services, async patterns |
| `cicd-automation-workflow-automate` | GitHub Actions CI/CD pipelines, Docker builds, release automation |
| `github-actions-templates` | Production-ready GitHub Actions workflow templates for Docker registries |

## CI/CD & Docker Publishing Skills

### cicd-automation-workflow-automate
**Purpose:** Create efficient CI/CD pipelines, GitHub Actions workflows for Docker image builds, and automated publishing to Docker Hub.

**Use Cases:**
- Build and push Docker images on push/release
- Multi-stage build pipelines with caching
- Docker image security scanning (Trivy, Snyk)
- Semantic versioning and release automation
- Deploy to cloud container services (ECS, EKS, AKS, GKE)

**Example Invocations:**
- "Create a GitHub Actions workflow to build and push Docker images to Docker Hub on release"
- "Set up CI pipeline with Docker build, test, and security scan stages"
- "Automate Docker image versioning and manifest management"

**Resources:**
- `resources/implementation-playbook.md` - Detailed workflow patterns with Docker examples

### github-actions-templates
**Purpose:** Production-ready GitHub Actions workflow patterns for testing, building, and deploying applications with Docker support.

**Use Cases:**
- Build and push Docker images to GHCR or Docker Hub
- Multi-platform Docker builds with cache optimization
- Security scanning with Trivy
- Matrix builds for multi-architecture images
- Kubernetes deployment workflows

**Example Invocations:**
- "Create a Docker build-push workflow with metadata extraction and cache optimization"
- "Set up a Rust CI pipeline with cargo test and clippy linting"
- "Build multi-architecture Docker images for linux/amd64 and linux/arm64"

**Reference Files:**
- `assets/deploy-workflow.yml` - Deployment workflow template
- `assets/matrix-build.yml` - Matrix build template

**GitHub Actions Workflow Template for Docker Hub:**
```yaml
# .github/workflows/docker-publish.yml
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

      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          push: ${{ github.event_name != 'pull_request' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

## Related Documentation

| Document | Purpose |
|----------|---------|
| [README.md](README.md) | Project overview, quick start, API documentation |
| [DEPLOY.md](DEPLOY.md) | Full deployment guide (local, Docker, production) |
| [PLAN.md](PLAN.md) | Detailed implementation plan with phases |
| [TODO.md](TODO.md) | Task tracking with completion status |

## Tech Stack

| Component | Technology | Version |
|-----------|------------|---------|
| Web Framework | Axum | 0.8 |
| Async Runtime | Tokio | 1.x |
| Model Inference | model2vec-rs | 0.1 |
| Caching | Moka (TTLCache) | 0.12 |
| Config | dotenvy | 0.15 |
| Serialization | serde + serde_json | 1.0 |
| Validation | validator | 0.16-0.20 |
| HTTP Auth | tower-http | 0.6 |

## Project Architecture

```
model2vec-rs-api-server/
├── Cargo.toml              # Rust dependencies
├── src/
│   ├── main.rs             # Entry point
│   ├── config/
│   │   └── mod.rs          # Configuration loading
│   ├── vectorizer/
│   │   └── mod.rs          # Model loading + caching (Moka TTLCache)
│   ├── app/
│   │   ├── mod.rs          # App state & router
│   │   ├── routes.rs       # HTTP endpoints
│   │   ├── models.rs       # Request/Response types
│   │   └── auth.rs         # Bearer token auth
│   └── lib.rs
├── Dockerfile              # Multi-stage Docker build
├── docker-compose.yml      # Container orchestration
├── .env                   # Environment variables
└── models/                 # Downloaded model files
```

## API Endpoints

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/.well-known/live` | GET | No | Health check (204) |
| `/.well-known/ready` | GET | No | Readiness check (204) |
| `/meta` | GET | Optional | Model metadata |
| `/v1/models` | GET | Optional | List available models |
| `/v1/embeddings` | POST | Optional | Generate embeddings |

## Key Mappings

| Python | Rust |
|--------|------|
| FastAPI | Axum |
| StaticModel (model2vec) | StaticModel (model2vec-rs) |
| TTLCache (cachetools) | Moka Cache |
| python-dotenv | dotenvy |
| Pydantic | serde + validator |
| base64 encoding | base64 crate |

## Performance Target

| Metric | Python | Rust |
|--------|--------|------|
| RAM Usage | ~1.2 GB | ~200-400 MB |
| Throughput | 4650 samples/s | 8000 samples/s |
