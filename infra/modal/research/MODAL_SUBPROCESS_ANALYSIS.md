# Modal Subprocess Lifecycle: Technical Deep-Dive

**Purpose:** Detailed analysis of Python subprocess patterns in Modal's web_server decorator
**Audience:** Rust/Python integration engineers
**Date:** 2026-03-11

---

## 1. Current Implementation Analysis

### 1.1 Your modal_deploy.py Pattern

```python
@app.function(
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
@modal.web_server(port=int(cfg("PORT")))
def serve() -> None:
    env = os.environ.copy()
    env.update(build_env())
    proc = subprocess.Popen(["/app/model2vec-api"], env=env)  # ← KEY LINE
    proc.wait()
```

### 1.2 What Happens When This Runs

**Timeline:**

```
T=0:  Modal starts Python function serve()
      ├─ Python runtime initialized
      ├─ Modal's heartbeat client starts
      ├─ Modal's async event loop running
      └─ HTTP request router initialized

T=0.1: serve() executes
      ├─ env = os.environ.copy()
      ├─ env.update(build_env())
      ├─ proc = subprocess.Popen(["/app/model2vec-api"], env=env)
      │  ├─ Forks child process (Rust binary)
      │  └─ Returns immediately (non-blocking)
      └─ proc.wait()  ← BLOCKS HERE INDEFINITELY

T=0.2: Rust binary initializes
      ├─ Listens on 0.0.0.0:8080
      ├─ Begins eager/lazy model loading
      └─ If eager: takes 8-42 seconds

T=10: (assuming 10s model load)
      ├─ Rust server ready
      ├─ HTTP router can now forward requests to port 8080
      ├─ Python still blocked in proc.wait()
      └─ Modal heartbeat loop: ??? (depends on event loop behavior)

T=120-600: Heartbeat timeout or health check timeout occurs
      ├─ Modal stops receiving heartbeats from Python
      ├─ OR Modal health check probe times out
      ├─ Modal kills the container ("Runner terminated")
      └─ Container lifecycle resets; repeat from T=0
```

---

## 2. The Blocking vs. Non-Blocking Dilemma

### 2.1 Why You Need to Block

**Problem:** Modal's web_server decorator expects the Python function to **keep running**

If you don't block, Modal thinks the function is done:

```python
@modal.web_server(port=8080)
def serve() -> None:
    proc = subprocess.Popen(["/app/model2vec-api"], env=env)
    # Function returns immediately
    # Modal: "serve() is done, function exited"
    # Container: "no active process, shut down"
    # Result: Rust server runs for a moment, then container dies
```

**So you must block.** The question is: **how?**

### 2.2 Blocking Options

#### Option A: `subprocess.run()` (Fully Blocking)

```python
subprocess.run(["/app/model2vec-api"], env=env, check=False)
```

**Behavior:**
- Waits for process to complete
- Captures stdout/stderr
- Returns process exit code

**Problem:** Identical to `proc.wait()` in terms of blocking

#### Option B: `subprocess.Popen().wait()` (What You Do)

```python
proc = subprocess.Popen(["/app/model2vec-api"], env=env)
proc.wait()
```

**Behavior:**
- Non-blocking fork
- Blocking wait (indefinite)
- Process runs in background while Python is blocked

**Problem:** Python event loop may be starved during blocking

#### Option C: `subprocess.Popen()` + Custom Loop (Most Flexible)

```python
import asyncio

proc = subprocess.Popen(["/app/model2vec-api"], env=env)

# Option C.1: Polling loop with yield points
while proc.poll() is None:
    await asyncio.sleep(0.1)  # Yield to event loop
```

**Behavior:**
- Forks subprocess
- Checks if still alive (non-blocking poll)
- Yields control to event loop periodically
- Allows Modal's heartbeat client to run

**Benefit:** Event loop stays responsive; heartbeat can run

#### Option D: Don't Wait (Race Condition)

```python
proc = subprocess.Popen(["/app/model2vec-api"], env=env)
# Function returns immediately
```

**Problem:** Container exits when function completes

---

## 3. The Heartbeat Loop Hypothesis

### 3.1 What We Know (From Modal Docs)

- Modal runs a **heartbeat loop** in the Python client
- This loop periodically sends pulses to Modal's host
- If heartbeating stops for **"a long period (minutes)"**, the container is terminated
- The heartbeat runs in the **Python runtime**

### 3.2 The Key Question

**Does `proc.wait()` block the Python event loop?**

| Scenario | Outcome |
|----------|---------|
| Heartbeat runs on main thread, `proc.wait()` blocks main thread | ❌ Heartbeat starves → timeout |
| Heartbeat runs on separate thread, `proc.wait()` blocks main thread | ⚠️ Heartbeat survives, but might be starved |
| Heartbeat runs on async event loop, `proc.wait()` blocks main thread | ❌ Event loop blocked → timeout |

**Evidence from your logs:**
- "Runner terminated" after ~1-5 minutes of model loading
- This timing matches a **heartbeat timeout**, not a startup timeout
- Variable load times suggest resource contention (heartbeat struggling to run)

### 3.3 Why Modal Documentation Doesn't Clarify This

Modal's documentation states:
> "The Modal client in `modal.Function` containers runs a heartbeat loop that the host uses to healthcheck the container's main process."

But it **does not specify:**
- Which thread the heartbeat runs on
- How blocking code affects the heartbeat
- Whether async event loop is required
- What exact timeout triggers container kill

This is a **critical documentation gap**.

---

## 4. Inference: Most Likely Scenario

Based on Modal's architecture (Python SDK, async-first design), **most likely**:

1. Modal's heartbeat runs in a **separate thread** (threading.Thread)
2. `proc.wait()` blocks the **main thread** (where Python user code runs)
3. The heartbeat thread **can** continue, but may have lower priority
4. If the heartbeat thread fails to send pulses for **30–120 seconds**, Modal kills the container

**Why this causes your problem:**
- Modal load-balancer or health checker calls `GET /.well-known/live`
- Request arrives while Python is blocked in `proc.wait()`
- Request is queued but not processed (main thread is blocking)
- After 30–120 seconds, health check times out
- Modal assumes container is dead → terminates it
- Rust process keeps running in background (orphaned)
- Container lifecycle resets

---

## 5. Why Local docker-compose Works

```yaml
services:
  model2vec-modal:
    image: docker.io/tys203831/model2vec-rs-api-server:modal
    ports:
      - "8080:8080"
```

**Key Difference:** No Python wrapper

```
Docker starts Rust binary directly (from Dockerfile ENTRYPOINT)
  ↓
Rust listens on 0.0.0.0:8080
  ↓
No Python runtime, no heartbeat loop, no event loop
  ↓
Docker-compose's health check (if any) is simple: test port 8080 open
  ↓
No timeout issues because no complex health check logic
```

**What's Different in Modal:**
- Modal injects a Python layer
- Python layer must manage the container's lifecycle
- Python's heartbeat must keep running

---

## 6. Solutions & Trade-offs

### Solution 1: Async Subprocess Polling

```python
import asyncio
import subprocess

@modal.web_server(port=8080)
async def serve() -> None:
    env = os.environ.copy()
    env.update(build_env())
    proc = subprocess.Popen(["/app/model2vec-api"], env=env)

    # Polling with event loop yielding
    while proc.poll() is None:
        await asyncio.sleep(0.1)
```

**Benefit:** Yields to event loop; heartbeat can run
**Risk:** `subprocess.Popen()` is not async-native; polling is inefficient
**Complexity:** Low

---

### Solution 2: Use @app.cls() with Lifecycle Hooks

```python
@app.cls(
    image=IMAGE,
    cpu=float(cfg("MODAL_CPU")),
    memory=int(cfg("MODAL_MEMORY_MB")),
    timeout=int(cfg("MODAL_TIMEOUT_SECS")),
    startup_timeout=int(cfg("MODAL_STARTUP_TIMEOUT_SECS")),
)
class ModelAPI:
    proc: subprocess.Popen = None

    @modal.enter()
    async def startup(self):
        """Called once per container at startup."""
        env = os.environ.copy()
        env.update(build_env())
        self.proc = subprocess.Popen(["/app/model2vec-api"], env=env)

        # Wait for Rust server to be ready
        import time
        time.sleep(2)

    @modal.exit()
    async def shutdown(self):
        """Called once per container at exit."""
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=30)
            except subprocess.TimeoutExpired:
                self.proc.kill()

    @modal.web_server(port=8080)
    async def serve(self):
        """HTTP request handler."""
        # This method doesn't actually handle requests
        # Modal routes directly to port 8080
        # But we need this method to be the web endpoint

        # Keep alive by periodic yielding
        while True:
            await asyncio.sleep(60)
```

**Benefit:** Separates startup from request handling; cleaner lifecycle
**Risk:** Requires refactoring Modal app structure
**Complexity:** Medium

**Key Insight:** `@modal.enter()` is the **right place** for initialization that takes time. It runs before the function is ready to handle requests, so timeouts don't apply to it (startup_timeout is used instead).

---

### Solution 3: Add Explicit Threading

```python
import threading

@modal.web_server(port=8080)
def serve() -> None:
    env = os.environ.copy()
    env.update(build_env())
    proc = subprocess.Popen(["/app/model2vec-api"], env=env)

    # Run in separate thread
    wait_thread = threading.Thread(target=proc.wait, daemon=False)
    wait_thread.start()

    # Keep main thread alive and responsive
    import time
    while True:
        time.sleep(1)
        if not wait_thread.is_alive():
            break  # Subprocess exited, exit too
```

**Benefit:** Main thread stays responsive, heartbeat can run
**Risk:** Additional threading complexity; may not help if heartbeat is also on main thread
**Complexity:** Low-Medium

---

### Solution 4: Direct Native Python HTTP Server

Instead of wrapping a Rust subprocess, embed the API logic in Python:

```python
import fastapi

app_fastapi = fastapi.FastAPI()

@app_fastapi.get("/.well-known/live")
async def live():
    return {"status": "ok"}

@modal.web_endpoint()
async def api():
    return await app_fastapi(...)
```

**Benefit:** No subprocess complexity; Modal's native web endpoint support
**Risk:** Requires porting Rust logic to Python (huge effort)
**Complexity:** Very High (probably not worth it)

---

## 7. Recommended Approach

### For Immediate Stability (No Code Change)

**Action:** Deploy with your current setup, but monitor with Modal CLI:

```bash
modal container list
modal container logs <container-id>
```

**What to Look For:**
- Does heartbeat timeout appear?
- Do health checks timeout?
- What's the exact error message?

**If logs show heartbeat timeout:** Proceed to Solution 1 (async polling)

### For Robust Long-Term Fix (Recommended)

**Action:** Refactor to Solution 2 (`@app.cls()` with lifecycle hooks)

**Rationale:**
- Separates startup (slow) from request handling (fast)
- Aligns with Modal's design patterns
- Simplest async integration
- Cleanest separation of concerns

**Effort:** 30–60 minutes of refactoring

---

## 8. Verification Checklist

After deploying a fix, verify:

- [ ] Container stays alive for >5 minutes without crashing
- [ ] `modal container logs` shows no "Runner terminated"
- [ ] Health checks return 204 within <1 second
- [ ] First embeddings request takes ~180 seconds (with lazy loading)
- [ ] Second and subsequent requests take <1 second
- [ ] No heartbeat timeout errors in logs

---

## 9. References for Further Reading

1. **Modal's `@modal.enter()` Pattern** — Best practice for long initialization
   - Docs: https://modal.com/docs/guide/lifecycle-functions

2. **Modal's `@modal.web_server()` vs `@modal.web_endpoint()`**
   - When to use each: https://modal.com/docs/guide/webhooks

3. **Python subprocess module**
   - Best practices: https://docs.python.org/3/library/subprocess.html

4. **Async/await in Python**
   - Modal supports async in web handlers
   - Docs: https://modal.com/docs/guide/async

---

## 10. Key Takeaways

1. **Blocking `proc.wait()` is necessary** to keep the container alive
2. **But it may starve Modal's heartbeat loop**, causing timeouts
3. **The fix is either:**
   - Use async polling (simple, low-risk)
   - Use `@app.cls()` lifecycle hooks (best practice, medium-risk)
4. **Modal's documentation has a critical gap:** it doesn't explain how blocking code interacts with the heartbeat loop
5. **Your local docker-compose works because there's no Python layer** — it's a direct comparison point

---

**End of Technical Deep-Dive**
