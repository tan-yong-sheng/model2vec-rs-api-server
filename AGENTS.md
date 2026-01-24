# AGENTS.md - Model2Vec Rust Migration

## Skills Required

| Skill | Purpose |
|-------|---------|
| `model2vec-rs-migration` | Core migration skill with step-by-step checklist and examples |

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
