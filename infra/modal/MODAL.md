# Modal Deployment (CPU-only)

This guide deploys the Rust API on Modal with a persistent Hugging Face cache to reduce cold starts.

## Files

- `infra/modal/modal_deploy.py` - Modal app entrypoint
- `infra/modal/setup_modal.sh` - Optional setup helper
- `infra/modal/.env.modal.example` - Configuration template

## Quick Start

```bash
# 1) Install + authenticate Modal
pip install modal
modal token new

# 2) Create config
cp infra/modal/.env.modal.example infra/modal/.env.modal

# 3) Deploy
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
```

## Why this setup

- CPU-only containers by default
- Persistent HF cache volume to avoid re-downloading models
- Small resource defaults for cost efficiency

## Configuration

Edit `infra/modal/.env.modal`.

### Model

- `MODEL_NAME` - Hugging Face model ID
- `ALIAS_MODEL_NAME` - Optional alias in API responses

### Modal resources

- `MODAL_CPU` (default `0.25`)
- `MODAL_MEMORY_MB` (default `1024`)
- `MODAL_MIN_CONTAINERS` (default `0`)
- `MODAL_MAX_CONTAINERS` (default `5`)
- `MODAL_SCALEDOWN_WINDOW` (default `300` seconds)
- `MODAL_IMAGE` (optional, e.g. `docker.io/owner/repo:tag`)
- `MODAL_ADD_PYTHON` (default `3.11`, needed for distroless images)

### Cold start reduction

- Persistent cache volume (default):
  - `MODAL_VOLUME_NAME=model2vec-hf-cache`
  - `MODAL_HF_CACHE_DIR=/data/hf`
- For fewer cold starts, raise `MODAL_SCALEDOWN_WINDOW` or set `MODAL_MIN_CONTAINERS=1`.

### App limits

- `CONCURRENCY_LIMIT` (default `16` for small CPU)
- `REQUEST_TIMEOUT_SECS` (default `30`)
- `MAX_INPUT_ITEMS`, `MAX_INPUT_CHARS`, `MAX_TOTAL_CHARS`

## Useful commands

```bash
# Deploy
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py

# Logs
modal logs -a model2vec-api --follow

# Status
modal status
```

## Using a prebuilt Docker Hub image

If you already publish a public image to Docker Hub, you can skip Modal’s build step.

```bash
MODAL_IMAGE=docker.io/tys203831/model2vec-rs-api-server:main
MODAL_ADD_PYTHON=3.11
```

This is faster for deploys (no build on Modal), but you still pay for image pull
and model downloads. Modal’s “fast image pull” via eStargz supports Docker Hub.
