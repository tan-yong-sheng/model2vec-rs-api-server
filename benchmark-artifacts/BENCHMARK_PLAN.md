# PR #1 Review & Benchmark Plan

## PR Overview
**Title:** Add lazy loading and automatic model unloading for memory management
**Author:** Copilot SWE Agent
**Branch:** `copilot/study-lazy-loading-feasibility`

### Key Changes
1. **Lazy Loading** (`LAZY_LOAD_MODEL`): Delays model init until first request
2. **Model Unloading** (`MODEL_UNLOAD_ENABLED`): Auto-unloads model after idle timeout
3. **Architecture Change**: Replaces `OnceCell` with `RwLock<Option<Arc<...>>>` for mutable storage

## Phase 1: PR Code Review

### Code Quality Assessment
- [x] Review diff for correctness
- [ ] Check for race conditions in double-check locking pattern
- [ ] Verify memory safety of Arc/RwLock usage
- [ ] Review error handling paths

### Configuration Changes
- [x] New env vars: `LAZY_LOAD_MODEL`, `MODEL_UNLOAD_ENABLED`, `MODEL_UNLOAD_IDLE_TIMEOUT`
- [x] Updated `.env.example`, `README.md`, `DEPLOY.md`

### Potential Issues Identified
1. **Race Condition Risk**: The `unload_vectorizer()` method acquires two write locks sequentially, not atomically
2. **Static Cache Invalidation**: Static `VECTORIZER` cache may cause issues across multiple AppState instances
3. **Memory Pressure**: No explicit memory reclamation - relies on Rust drop semantics

## Phase 2: GitHub Actions Workflow Test

### Current Workflow Analysis
- Builds multi-platform images (linux/amd64, linux/arm64)
- Triggers on: push tags (v*), PRs to main, workflow_dispatch
- Pushes to Docker Hub (requires secrets)
- Uses build cache (gha)

### Test Plan
1. [ ] Check if workflow runs successfully on PR
2. [ ] Verify build completes without errors
3. [ ] Note: Cannot push to Docker Hub without credentials

## Phase 3: Performance Benchmarking

### Test Environment
- Model: minishlab/potion-base-8M (default) or larger if available
- Hardware: Current Oracle Cloud instance
- Measurement tools: curl, time command, Docker stats

### Benchmark Scenarios

#### Scenario A: Baseline (No Unloading)
```bash
LAZY_LOAD_MODEL=false
MODEL_UNLOAD_ENABLED=false
```
- Measure: Startup time, memory at startup, memory after 5 min idle
- Request latency: p50, p95, p99

#### Scenario B: Lazy Loading Only
```bash
LAZY_LOAD_MODEL=true
MODEL_UNLOAD_ENABLED=false
```
- Measure: Startup time (should be instant), first request latency
- Memory after first request

#### Scenario C: Model Unloading (THE KEY TEST)
```bash
LAZY_LOAD_MODEL=false
MODEL_UNLOAD_ENABLED=true
MODEL_UNLOAD_IDLE_TIMEOUT=30  # 30 seconds for quick test
```
- Measure:
  1. Startup time
  2. Memory at startup
  3. Request latency (baseline)
  4. Memory after 30s idle
  5. **First request after unload (CRITICAL)** - how long?
  6. Memory after reload

#### Scenario D: Combined (Lazy + Unload)
```bash
LAZY_LOAD_MODEL=true
MODEL_UNLOAD_ENABLED=true
MODEL_UNLOAD_IDLE_TIMEOUT=30
```
- Measure combined effects

### Decision Criteria

Based on user's requirements:
- **Memory savings must be significant** (target: >80% reduction when idle)
- **First request after unload MUST be < 30 seconds** to be acceptable
- If reload time > 30s: Document as unacceptable for production use
- If reload time 2+ min: Feature is only suitable for dev environments

### Expected Results (from PR description)
- 128M model: 250MB active → 20MB idle (92% savings)
- Reload time: ~150s (2.5 min) - THIS EXCEEDS 30s THRESHOLD

## Phase 4: Documentation

### Deliverables
1. Code review findings
2. Workflow test results
3. Benchmark report with:
   - Actual memory usage numbers
   - Actual latency numbers (especially reload time)
   - Recommendation on whether to merge
4. Usage guidelines based on findings

## Test Commands

```bash
# Build Docker image
docker build -t model2vec-benchmark .

# Run with monitoring
docker run -d --name model2vec-test \
  -p 8080:8080 \
  -e MODEL_NAME=minishlab/potion-base-8M \
  -e MODEL_UNLOAD_ENABLED=true \
  -e MODEL_UNLOAD_IDLE_TIMEOUT=30 \
  model2vec-benchmark

# Monitor memory
docker stats model2vec-test --no-stream

# Test request
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"input": "test", "model": "test-model"}'

# Time the request
time curl -X POST http://localhost:8080/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"input": "test", "model": "test-model"}'
```

## Decision Matrix

| Reload Time | Recommendation |
|-------------|----------------|
| < 10s | Excellent - merge with confidence |
| 10-30s | Acceptable - merge with warnings |
| 30-60s | Marginal - document as dev-only feature |
| > 60s | Poor - likely reject or require significant changes |
| > 120s | Unacceptable - reject PR |

---
*Plan created for PR #1 review and benchmark*
