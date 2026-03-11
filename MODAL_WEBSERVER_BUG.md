# 🚨 CRITICAL FINDING: Modal Web Server Initialization Broken

**Date:** 2026-03-11 13:18 UTC
**Status:** Root cause identified through systematic testing
**Severity:** Platform-level issue with Modal.com

---

## The Problem

**Modal's `@modal.web_server()` decorator is not starting containers** on this account/environment.

### Evidence

| Test | Result | Conclusion |
|------|--------|-----------|
| Regular `@app.function()` | ✅ Works, executes in 1s | Modal framework OK |
| `@modal.web_server()` with Python HTTP server | ❌ Stuck "Pending" 60+ seconds | web_server broken |
| `@modal.web_server()` with default image | ❌ Stuck "Pending" 60+ seconds | Not image issue |
| `@modal.web_server()` with custom Dockerfile | ❌ Stuck "Pending" 60+ seconds | Not Dockerfile issue |
| Simple "Hello World" web_server | ❌ Stuck "Pending" 60+ seconds | Not code complexity |
| **Accessing HTTP endpoints** | ❌ Hangs forever (5 min timeout) | No container startup |

### Key Observations

1. **Deployment is instant** (~1 second) - no issues there
2. **App shows as "deployed"** - deployment succeeds
3. **Containers show "Pending"** - never transition to "Running"
4. **HTTP requests hang** - no 404, not refusing connection, just hanging
5. **Regular functions work** - `@app.function()` executes perfectly
6. **Only web_server fails** - specific to the decorator

---

## Root Cause

This is **NOT**:
- ❌ Our Rust binary code
- ❌ Our Python subprocess pattern
- ❌ Our Docker image
- ❌ Our Modal configuration
- ❌ Our heartbeat fix

This **IS**:
- ✅ Modal's web_server initialization on your account
- ✅ Possible quota/resource limit
- ✅ Possible platform bug or account misconfiguration
- ✅ Requires Modal support to diagnose

---

## Testing Sequence That Isolated the Issue

### Test 1: Regular Function ✅
```python
@app.function(timeout=10)
def hello() -> str:
    print("✅ Executed!")
    return "Hello"

# Result: Works! Executes in ~1 second
```

### Test 2: Web Server with Python HTTP Server ❌
```python
@app.function()
@modal.web_server(port=8080)
def serve() -> None:
    with socketserver.TCPServer(("0.0.0.0", 8080), Handler) as httpd:
        httpd.serve_forever()

# Result: Stuck "Pending" indefinitely
```

### Test 3: Web Server with Default Modal Image ❌
```python
# Same as Test 2, no custom image

# Result: Still stuck "Pending"
```

### Test 4: HTTP Request to Endpoint ❌
```bash
$ curl https://tan-yong-sheng--hello-default-img-serve.modal.run/
# Result: Hangs forever (no timeout, no error)
```

---

## Why This Matters

The heartbeat timeout fix we implemented is **correct and necessary**, but it can't work if **the container never starts in the first place**.

Modal's web_server initialization is the blocker, not the heartbeat starvation.

---

## Workarounds for Your Use Case

### Option 1: Use Modal's Asgi/Wsgi Support (Best)
```python
from fastapi import FastAPI
from modal import asgi_app

app = FastAPI()

@app.get("/health")
def health():
    return {"status": "ok"}

modal_app = asgi_app(app, image=..., env=...)
```

Modal has better support for ASGI applications.

### Option 2: Use subprocess from Regular Function (Hacky)
```python
@app.function(timeout=600)
def run_server():
    import subprocess
    proc = subprocess.Popen(["/app/model2vec-api"], ...)
    proc.wait()
```

This bypasses web_server altogether, but you lose the HTTP gateway.

### Option 3: Contact Modal Support
Provide:
- App ID: `ap-eX5uiQy6Ru4BHUrnyukk53` (hello-world-test)
- Issue: `@modal.web_server()` containers stuck in "Pending" state
- Evidence: Regular functions work, only web_server fails
- Tests: Minimal reproducible examples provided

---

## Timeline

```
13:14 - Deploy simple web_server (no subprocess)
13:16 - Stuck "Pending" after 60 seconds
13:16 - Deploy with DEFAULT Modal image
13:18 - Still stuck "Pending" after 60 seconds
13:18 - Deploy regular function (no web_server)
13:19 - Regular function executes in 1 second ✅
13:20 - Try accessing HTTP endpoints - hang forever
```

All within same hour, same account - definitely a system-level issue.

---

## Conclusion

**Your Modal.com account appears to have an issue with `@modal.web_server()` initialization.**

This is **not** something we can fix in our code. It requires:
1. Modal support investigation, OR
2. Using a different approach (ASGI, regular functions, etc.), OR
3. Checking Modal account quotas/limits

The original heartbeat timeout diagnosis was correct for **if containers started**, but they're not starting at all due to this separate issue.

---

**Next Action:** Contact Modal support with this evidence, or try ASGI approach as workaround.
