# Modal Deployment Strategies Research

**Status:** Investigation Phase
**Date:** 2026-03-11
**Goal:** Evaluate best approach for stateless HTTP service (model2vec-rs Rust API)

---

## Research Findings

### 1. Modal Web Server Pattern (Current Approach)

**What Modal Actually Does:**
- `@modal.web_server` decorator runs the function and keeps it alive
- The function itself should spawn a subprocess and **block indefinitely** on it
- Modal routes HTTP requests to the port specified in the decorator
- The function body must never return (or container exits)

**Our Current Implementation (Lifecycle Hooks):**
```python
@app.cls(...)
class ModelAPI:
    @modal.enter()  # Called once at startup
    def startup(self): ...

    @modal.web_server(port=8080)  # Never returns?
    def serve(self): ...

    @modal.exit()  # Called at shutdown
    def shutdown(self): ...
```

**The Problem:**
- `@modal.enter()` is NOT for slow operations - it's for initialization
- The `serve()` method in a class-based app might have different lifecycle rules
- The HTTP routing might expect a specific function signature

### 2. Sandbox Alternative

**What Sandboxes Are:**
- Dynamic containers that can run arbitrary commands
- Designed for "long-lived services in the background"
- Extended timeout support (up to 24 hours)
- More flexible than Function decorator
- Can execute custom shell commands directly

**Advantages for Stateless Apps:**
- No Python wrapper needed - just run Rust binary directly
- Simpler lifecycle management
- Better for long-running processes
- Can use Modal's HTTP tunneling features

### 3. Simple Web Server Pattern (Working in Production)

Modal's own examples (vLLM inference) use this pattern:

```python
@modal.web_server(port=8000)
def serve():
    """Simple blocking pattern that works."""
    subprocess.Popen(["/app/binary"], env=env).wait()
```

**Key Insight:** The function MUST block indefinitely. When it returns, Modal thinks the service is done.

---

## Recommendation

### Option A: Revert to Simple Pattern (Lowest Risk)

Use the simplest pattern that Modal documentation shows works:

```python
@app.function(
    image=IMAGE,
    # ... config ...
)
@modal.web_server(port=8080)
def serve():
    """Single function with blocking subprocess.wait()."""
    env = os.environ.copy()
    env.update(build_env())

    proc = subprocess.Popen(["/app/model2vec-api"], env=env)

    # This blocks indefinitely, which is what Modal expects
    returncode = proc.wait()

    # If we get here, subprocess exited (shouldn't happen in normal operation)
    if returncode != 0:
        raise RuntimeError(f"Server exited with code {returncode}")
```

**Pros:**
- Matches Modal's own working examples
- Simpler than lifecycle hooks
- No async complexity
- Modal-tested pattern

**Cons:**
- Still uses blocking wait() which may cause heartbeat issues
- Need monitoring to detect if container crashes

### Option B: Use Modal Sandboxes (Most Elegant)

```python
from modal import Sandbox

@app.function()
def run_service():
    """Run Rust binary in a sandboxed container."""
    sandbox = Sandbox.create(
        image=IMAGE,
        mounts=[/* hf volume mount */],
        env=build_env(),
        timeout=86400,  # 24 hours
    )

    # Run the binary directly
    sandbox.run("cd /app && ./model2vec-api")
```

**Pros:**
- No Python wrapper at all - Rust runs directly
- Cleaner separation of concerns
- Modal's recommended pattern for long-running services
- Better timeout handling

**Cons:**
- Different deployment model than current
- May need HTTP tunneling setup
- Less documentation for this specific use case

### Option C: Use Synchronous Function Without Classes (Middle Ground)

```python
@app.function(
    image=IMAGE,
    timeout=1200,
    startup_timeout=1800,
    # ... other config ...
)
@modal.web_server(port=int(cfg("PORT")))
def serve():
    """Synchronous web server without class complexity."""
    env = os.environ.copy()
    env.update(build_env())

    proc = subprocess.Popen(
        ["/app/model2vec-api"],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True
    )

    # Simple polling with logging
    tick = 0
    while proc.poll() is None:
        tick += 1
        if tick % 20 == 0:
            print(f"✓ Service running ({tick * 0.5}s)")
        time.sleep(0.5)

    # Service exited
    print(f"✗ Service exited with code {proc.returncode}")
    raise RuntimeError(f"Service died with code {proc.returncode}")
```

**Pros:**
- Simpler than class-based approach
- Still allows polling (giving heartbeat some breathing room)
- Matches Modal examples more closely
- Easier to debug

**Cons:**
- May still have heartbeat starvation (but with time.sleep polling instead of blocking wait)
- Requires monitoring

---

## Decision Matrix

| Criteria | Simple Function | Lifecycle Hooks | Sandboxes |
|----------|-----------------|-----------------|-----------|
| Complexity | ⭐ Low | ⭐⭐⭐ High | ⭐⭐ Medium |
| Risk | ⭐⭐ Medium | ⭐⭐⭐⭐ High | ⭐ Low |
| Modal Support | ⭐⭐⭐⭐⭐ Excellent | ⭐⭐ Poor | ⭐⭐⭐ Good |
| Monitoring | ⭐⭐ Manual | ⭐⭐⭐ Better | ⭐⭐⭐ Good |
| Cold Start | ⭐⭐ ~5-30s | ⭐⭐ ~5-30s | ⭐ ~5s |

---

## Recommendation

**Use Option C: Synchronous Function (Simple Pattern)**

Rationale:
1. **Modal's own examples use this pattern** - If it works for vLLM, it works for us
2. **Simpler to debug** - No class complexity, straightforward control flow
3. **Lower risk** - We know this pattern works in Modal production deployments
4. **Good enough polling** - `time.sleep(0.5)` gives heartbeat chances to run
5. **Easier to monitor** - Single function = clearer logs and behavior

---

## Implementation Steps

1. **Revert to simple function-based pattern**
   - Remove class-based ModelAPI
   - Use single `@app.function()` + `@modal.web_server()` decorators
   - Keep polling approach (time.sleep, not blocking wait)

2. **Deploy and test**
   - Check container logs for startup messages
   - Test health endpoints
   - Monitor for 10 minutes to verify stability

3. **If still fails, escalate to Sandboxes**
   - Try running Rust binary directly without Python wrapper
   - Use Modal's native sandbox features

---

## Testing Checklist

- [ ] Deploy simple function version
- [ ] Wait 30 seconds for startup
- [ ] Test `/.well-known/live` - should return 204 in <1s
- [ ] Test `/.well-known/ready` - should return 204 in <1s
- [ ] Test `/v1/models` - should return 200 with JSON
- [ ] Monitor logs for first 5 minutes - no "Runner terminated"
- [ ] Check container resource usage - reasonable CPU/memory
- [ ] Leave running for 30 minutes - verify stability

---

## References

- Modal vLLM Example: https://modal.com/docs/examples/llm_inference
- Modal Sandboxes: https://modal.com/docs/guide/sandboxes
- Modal Web Servers: https://modal.com/docs/guide/webhooks
- Modal Lifecycle: https://modal.com/docs/guide/lifecycle-functions

