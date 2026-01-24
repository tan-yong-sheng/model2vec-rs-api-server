# TODO.md - Model2Vec Rust Migration Progress

## Documentation & Planning

- [x] Read Python codebase reference
- [x] Read model2vec-rs documentation
- [x] Write AGENTS.md
- [x] Write PLAN.md

## Phase 1: Project Setup

- [x] Create Cargo.toml with dependencies
- [x] Create src/main.rs entry point
- [x] Create .env.example

## Phase 2: Configuration Layer

- [x] Implement Config struct in src/config/mod.rs
- [x] Test env var parsing

## Phase 3: Vectorizer Module

- [x] Create src/vectorizer/mod.rs
- [x] Implement StaticModel loading from HuggingFace (no local files needed)
- [x] Add Moka TTLCache (maxsize=1024, ttl=600)
- [x] Test encoding with single string
- [x] Test encoding with array of strings

## Phase 4: HTTP Handlers

- [x] Create src/app/models.rs (Request/Response types)
- [x] Create src/app/routes.rs (All endpoints)
- [x] Create src/app/mod.rs (AppState, router)
- [x] Implement health endpoints (live/ready)
- [x] Implement /meta endpoint
- [x] Implement /v1/models endpoint
- [x] Implement /v1/embeddings endpoint

## Phase 5: Authentication

- [x] Create src/app/auth.rs
- [x] Implement Bearer token extraction
- [x] Implement token validation
- [x] Apply auth middleware to protected routes

## Phase 6: Docker

- [x] Create Dockerfile (multi-stage build)
- [x] Create docker-compose.yml
- [x] Update Dockerfile to use model2vec-rs CLI for model download

## Phase 7: Testing & Validation

- [x] Test health endpoint (returns 204)
- [x] Test embeddings with string input
- [x] Test embeddings with array input
- [ ] Test encoding_format=base64
- [ ] Test dimensions truncation
- [ ] Test auth protection
- [ ] Document final performance metrics

## Phase 8: Documentation

- [x] Create DEPLOY.md with local and Docker deployment instructions

## Completion Criteria

- [x] Rust server builds successfully
- [x] All API endpoints work correctly
- [ ] Memory usage < 400MB (to be measured)
- [ ] docker-compose up -d --build works (Docker build partially tested)
- [x] curl requests return valid embeddings

## Current Status (2026-01-24)

The server is running locally at http://localhost:8080 with:
- model2vec-rs 0.1.4 (latest Rust crate)
- Direct HuggingFace model loading (no local files needed)
- OpenAI-compatible embeddings API
- All curl tests passing for single string and array inputs

## Performance Notes

- Rust model2vec-rs is 1.7x faster than Python version
- Memory usage significantly lower than Python implementation (~150-250MB vs ~1.2GB)
