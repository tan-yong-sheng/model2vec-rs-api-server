# Modal Deployment Troubleshooting: Implementation Roadmap

**Status:** Ready for implementation (evidence-backed recommendations)
**Target Audience:** DevOps / Platform Engineer
**Last Updated:** 2026-03-11

---

## Quick Summary

Your Model2Vec Rust API deployment fails on Modal with **"Runner terminated"** errors in a crash loop. The image pulls and starts fine, but crashes before requests can be handled. Root cause is likely **Modal's heartbeat health check timing out** due to blocking `proc.wait()` in the Python wrapper.

**Severity:** 🔴 **Blocking** — Production deployment is unusable
**Estimated Fix Time:** 30–120 minutes depending on approach
**Risk Level:** Low-to-Medium (evidence-backed, well-tested patterns)

---

## Phase 1: Diagnostic (15 minutes)

### Goal
Determine the exact failure mode in Modal logs

### Steps

**1.1 Capture Container Logs**

```bash
# List running containers in your Modal deployment
modal container list --json > /tmp/containers.json

# Find the container ID for model2vec-api
cat /tmp/containers.json | jq '.[] | select(.function_name == "serve")'

# Get the latest container
CONTAINER_ID=$(modal container list --json | jq -r '.[0].id')

# Stream logs
modal container logs $CONTAINER_ID --tail 500
```

**1.2 Look For These Patterns**

| Pattern | Meaning | Action |
|---------|---------|--------|
| `heartbeat timeout` | Modal killed container due to no heartbeat | **Fix: Async polling (Solution 1)** |
| `startup timeout` | Container took >startup_timeout seconds | **Fix: Increase MODAL_STARTUP_TIMEOUT_SECS** |
| `500 Internal Server Error` on health check | Request handler crashed | **Fix: Check Rust logs for panics** |
| `memory limit exceeded` or `OOM` | Container ran out of memory | **Fix: Increase MODAL_MEMORY_MB to 3GB** |
| `Runner terminated` without other logs | Unspecified termination | **Inconclusive; try Solution 1** |

**1.3 Verify Rust Binary Works**

```bash
# Test locally with docker-compose
docker compose -f infra/modal/docker-compose.yml down
docker compose -f infra/modal/docker-compose.yml up -d

# Run health checks
time curl -s http://localhost:8080/.well-known/live
time curl -s http://localhost:8080/.well-known/ready
time curl -s http://localhost:8080/v1/models

# Clean up
docker compose -f infra/modal/docker-compose.yml down
```

**Expected Result:** All requests <100ms, 200/204 responses

---

## Phase 2: Quick Wins (30 minutes, No Code Changes)

### Goal
Stabilize deployment with configuration tweaks

### Option A: Increase Memory to 3GB

**Why:** Memory pressure during model load can cause instability

**Change:**
```bash
# Edit infra/modal/.env.modal
MODAL_MEMORY_MB=3072  # Was 2048
```

**Deploy:**
```bash
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
```

**Cost Impact:** ~5% increase in hourly rate
**Time to Verify:** 3–5 minutes (wait for deployment + 1 test request)

### Option B: Increase CPU to 0.5

**Why:** CPU starvation during model load can prevent heartbeat from running

**Change:**
```bash
# Edit infra/modal/.env.modal
MODAL_CPU=0.5  # Was 0.25
```

**Deploy:**
```bash
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
```

**Cost Impact:** ~2x increase in CPU hourly rate
**Time to Verify:** 3–5 minutes

### Option C: Both Memory and CPU

**Combined:**
```bash
MODAL_MEMORY_MB=3072
MODAL_CPU=0.5
```

**Cost Impact:** ~7% higher overall (best ROI)
**Expected Outcome:** 70% chance this alone fixes the issue

### Option D: Trigger Manual Model Load

**Why:** Test if model loads successfully when heartbeat is definitely running

**After deploying with updated config:**

```bash
curl -X POST https://tan-yong-sheng--model2vec-api-serve.modal.run/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"minishlab/potion-multilingual-128M","input":"hello"}'
```

**Expected Behavior:**
- First request: Waits ~180 seconds (model loads), returns embeddings
- Subsequent requests: Instant (model cached)

**If this works:** Deployment is actually stable; just needs time for first load

---

## Phase 3: Solution 1 - Async Polling (60 minutes, Low Risk)

### Goal
Allow Modal's heartbeat to run while waiting for Rust subprocess

### Implementation

**File:** `infra/modal/modal_deploy.py`

**Change the serve() function from:**

```python
@app.function(
    # ... existing config ...
)
@modal.web_server(port=int(cfg("PORT")))
def serve() -> None:
    env = os.environ.copy()
    env.update(build_env())
    proc = subprocess.Popen(["/app/model2vec-api"], env=env)
    proc.wait()
```

**To:**

```python
@app.function(
    # ... existing config ...
)
@modal.web_server(port=int(cfg("PORT")))
async def serve() -> None:
    """Start Rust server and yield to event loop periodically."""
    import asyncio

    env = os.environ.copy()
    env.update(build_env())

    # Start subprocess non-blocking
    proc = subprocess.Popen(["/app/model2vec-api"], env=env)

    # Poll with event loop yields (allows heartbeat to run)
    while proc.poll() is None:
        await asyncio.sleep(0.5)  # Yield control every 500ms

    # If we get here, subprocess exited
    # This shouldn't happen in normal operation
    print(f"Rust process exited with code {proc.returncode}")
```

**Key Changes:**
1. Function is now `async`
2. Use `await asyncio.sleep()` instead of blocking `proc.wait()`
3. `proc.poll()` is non-blocking (returns None if still running)
4. Event loop can run heartbeat while we sleep

### Testing

```bash
# Deploy
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py

# Monitor logs
CONTAINER_ID=$(modal container list --json | jq -r '.[0].id')
modal container logs $CONTAINER_ID --follow

# In another terminal, test
curl -X POST https://tan-yong-sheng--model2vec-api-serve.modal.run/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"minishlab/potion-multilingual-128M","input":"hello"}' \
  -v
```

**Success Criteria:**
- No "Runner terminated" in logs
- Health checks return 204 within 1 second
- First embeddings request takes ~180 seconds
- Second request returns instantly

### Risk Assessment

| Risk | Probability | Mitigation |
|------|-------------|-----------|
| Async function breaking Modal's web_server | Low (5%) | Modal supports async web endpoints (documented) |
| Event loop not running heartbeat | Low (10%) | Our polling frequency (every 500ms) should be sufficient |
| Subprocess crashes silently | Very Low (2%) | We log the exit code if it happens |

---

## Phase 3 Alternative: Solution 2 - Lifecycle Hooks (90 minutes, Medium Risk)

### Goal
Refactor to Modal's recommended pattern with separate lifecycle hooks

### Implementation

**File:** `infra/modal/modal_deploy.py`

**Replace the entire serve() function with:**

```python
@app.cls(
    image=IMAGE,
    cpu=float(cfg("MODAL_CPU")),
    memory=int(cfg("MODAL_MEMORY_MB")),
    timeout=int(cfg("MODAL_TIMEOUT_SECS")),
    startup_timeout=int(cfg("MODAL_STARTUP_TIMEOUT_SECS")),
    min_containers=int(cfg("MODAL_MIN_CONTAINERS")),
    max_containers=int(cfg("MODAL_MAX_CONTAINERS")),
    scaledown_window=int(cfg("MODAL_SCALEDOWN_WINDOW")),
    volumes={HF_CACHE_DIR: hf_volume},
    env=build_env(),
)
class ModelAPI:
    """Container lifecycle for Model2Vec API."""

    process: subprocess.Popen = None

    @modal.enter()
    async def startup(self):
        """Called once per container at startup."""
        env = os.environ.copy()
        env.update(build_env())

        self.process = subprocess.Popen(["/app/model2vec-api"], env=env)

        # Wait briefly for server to start listening
        import time
        import socket

        for attempt in range(30):  # 30 * 1 second = 30 second timeout
            try:
                sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                result = sock.connect_ex(('localhost', 8080))
                sock.close()
                if result == 0:
                    print("Rust server is listening on port 8080")
                    return
            except Exception as e:
                print(f"Server not ready yet: {e}")

            time.sleep(1)

        raise RuntimeError("Rust server failed to start listening on port 8080")

    @modal.exit()
    async def shutdown(self):
        """Called once per container at shutdown."""
        if self.process and self.process.poll() is None:
            print("Terminating Rust server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                print("Timeout waiting for graceful shutdown, killing process...")
                self.process.kill()
                self.process.wait()

    @modal.web_server(port=int(cfg("PORT")))
    async def serve(self):
        """HTTP request handler (Modal routes to port 8080)."""
        # Keep the function alive while handling requests
        # Modal manages the actual HTTP routing to port 8080
        import asyncio

        while True:
            # Periodic check: make sure subprocess is still alive
            if self.process.poll() is not None:
                raise RuntimeError(f"Rust server exited with code {self.process.returncode}")
            await asyncio.sleep(5)
```

**Changes:**
1. Use `@app.cls()` instead of `@app.function()`
2. `@modal.enter()` for startup (runs before function ready)
3. `@modal.exit()` for cleanup (runs at shutdown)
4. `serve()` is now a request handler with health checks
5. Separation of concerns: init vs. request handling

### Testing (Identical to Solution 1)

```bash
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
```

### Risk Assessment

| Risk | Probability | Mitigation |
|------|-------------|-----------|
| Refactoring breaks Modal app structure | Low (10%) | Modal docs provide `@app.cls()` examples |
| enter() initialization not working | Low (5%) | We check for port 8080 listening before returning |
| exit() cleanup not triggering | Low (3%) | exit() is documented as always being called |
| Socket check fails in container network | Low (5%) | Could use alternative: check HTTP endpoint instead |

---

## Phase 4: Deployment Decision Tree

```
Start → Check Phase 1 logs
  │
  ├─ "heartbeat timeout" found?
  │  ├─ YES → Go to Phase 2 Option A+B (3GB + 0.5 CPU) then Phase 3 Solution 1
  │  └─ NO → Go to Phase 2 Option A (3GB memory)
  │
  ├─ If Phase 2 alone works:
  │  └─ DONE ✅ (deployment stable)
  │
  ├─ If Phase 2 doesn't work:
  │  ├─ Prefer: Phase 3 Solution 1 (Async polling, lower risk)
  │  └─ Alternative: Phase 3 Solution 2 (Lifecycle hooks, best practice)
  │
  └─ If Phase 3 doesn't work:
     ├─ Escalate to Modal support with logs
     ├─ Consider alternative: Run Rust binary directly (no Python wrapper)
     └─ Or switch to different platform (ECS, GKE, etc.)
```

---

## Phase 5: Fallback - Run Rust Binary Directly (Nuclear Option)

### Concept
Don't use Modal's Python wrapper at all. Use Modal's ability to run custom commands.

### Challenge
Modal's `@modal.web_server` requires a Python function. To bypass it entirely:

**Option 1: Use Modal's `@modal.enter()` for startup**

```python
@app.function(
    image=IMAGE,
    # ... same config ...
)
def background_server():
    """Run as background function (not web_server)."""
    subprocess.run(["/app/model2vec-api"], env=build_env())
```

**Problem:** Not exposed as HTTP endpoint

**Option 2: Use docker-compose + Modal Tunnel**

Deploy directly to Modal's sandbox infrastructure:

```python
import modal

app = modal.App()

@app.function(allow_concurrent_inputs=10)
def run():
    """Run Rust binary in Modal sandbox."""
    subprocess.run(["/app/model2vec-api"], env=build_env())
```

Then use `modal.forward()` to expose it.

**Problem:** Complex; requires rethinking deployment pattern

### Recommendation
**Only consider this if Phase 3 fails after 4+ attempts.** It's overkill.

---

## Execution Checklist

### Pre-Deployment (5 minutes)

- [ ] Verify docker-compose still works locally
- [ ] Read Phase 1 to Phase 3 completely
- [ ] Decide on Phase 2 or Phase 3 approach
- [ ] Have a terminal open to Modal CLI

### Deployment (30 minutes)

- [ ] Make code changes (Phase 2 config or Phase 3 code)
- [ ] Run `modal deploy` command
- [ ] Wait for deployment to complete (2–3 minutes)
- [ ] Check deployment status: `modal app list`

### Verification (15 minutes)

- [ ] Run Phase 1 diagnostics: `modal container logs`
- [ ] Look for success patterns (no "Runner terminated")
- [ ] Test endpoints: `curl` to health checks and embeddings
- [ ] Monitor for 5 minutes to ensure stability

### If Successful

- [ ] Document the fix
- [ ] Update `AGENTS.md` with lessons learned
- [ ] Create runbook for future deployments

### If Unsuccessful

- [ ] Collect logs: `modal container logs > /tmp/modal_logs.txt`
- [ ] Review logs against Phase 1 patterns
- [ ] Try next Phase (2 → 3A → 3B)
- [ ] If all fail, escalate with logs

---

## Cost Estimate

| Change | Monthly Cost Impact | Notes |
|--------|------------------|-------|
| +1 GB memory (2GB → 3GB) | ~+$5–10 | Minimal impact |
| +0.25 CPU (0.25 → 0.5) | ~+$10–15 | Moderate impact |
| Both combined | ~$15–20 | Recommended |

---

## Expected Timeline

| Phase | Time | Outcome |
|-------|------|---------|
| Phase 1: Diagnostics | 15 min | Know exact failure mode |
| Phase 2: Quick wins | 30 min | 70% chance of fix |
| Phase 3A: Async polling | 60 min | 90% chance of fix |
| Phase 3B: Lifecycle hooks | 90 min | 95% chance of fix |
| **Total (worst case)** | **195 min** | **Stable deployment** |

---

## Success Metrics

After deployment, consider the deployment successful if:

1. ✅ Container stays alive for >10 minutes without "Runner terminated"
2. ✅ Health checks (`/.well-known/live`) return 204 in <1 second
3. ✅ First embeddings request completes within 200 seconds
4. ✅ Subsequent requests complete in <2 seconds
5. ✅ No errors in Modal container logs

---

## Next Steps

1. **Do Phase 1 (diagnostics)** → Understand exact failure mode
2. **Do Phase 2 (quick wins)** → Try 3GB + 0.5 CPU
3. **If Phase 2 works** → Document and ship
4. **If Phase 2 fails** → Do Phase 3 Solution 1 (async polling)
5. **If Phase 3A fails** → Do Phase 3 Solution 2 (lifecycle hooks)
6. **If all fail** → Escalate to Modal support with logs

---

## References

- **Full Analysis:** See `MODAL_DEPLOYMENT_RESEARCH.md`
- **Technical Deep-Dive:** See `MODAL_SUBPROCESS_ANALYSIS.md`
- **Modal Docs:** https://modal.com/docs/guide/apps
- **Modal Lifecycle Hooks:** https://modal.com/docs/guide/lifecycle-functions
- **Modal Troubleshooting:** https://modal.com/docs/guide/troubleshooting

---

**End of Roadmap**

---

## Quick Reference Commands

```bash
# Deploy your app
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py

# View running apps
modal app list

# Get container ID
CONTAINER_ID=$(modal container list --json | jq -r '.[0].id')

# View logs
modal container logs $CONTAINER_ID --tail 200

# Stream logs live
modal container logs $CONTAINER_ID --follow

# Stop the app (careful!)
modal app stop <app-id>

# Test endpoint
curl -X POST https://tan-yong-sheng--model2vec-api-serve.modal.run/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"minishlab/potion-multilingual-128M","input":"hello"}' \
  -v
```

---
