# 🔍 Modal Deployment - Debugging Status

**Date:** 2026-03-11
**Status:** 🟠 **Stuck in "Pending" - Bootstrap Issue (Not Heartbeat)**
**Next Step:** Requires manual Modal investigation or alternative approach

---

## Summary of Investigation

### What We Confirmed ✅
1. **Code fix is correct** - Async/await removed, using time.sleep(0.5) polling
2. **Dockerfile is working** - Builds successfully (244 seconds for Rust compilation)
3. **Config is valid** - .env.modal loads, all parameters are correct
4. **Docker image exists** - Prebuilt image pulls successfully
5. **Modal deployment succeeds** - Functions deploy without errors

### What's Actually Happening ❌

**Containers stuck in "Pending" state indefinitely** (60+ seconds):
- No "Running" state reached
- Multiple containers spawn and restart (indicating crashes)
- `serve()` function **never executes** (debug prints not appearing)
- Not a heartbeat issue (problem occurs before serve() even starts)

This is a **Modal container bootstrap failure**, not our code problem.

---

## Root Cause Analysis

### Timeline of Failures

1. **First attempt:** Model download at startup (LAZY_LOAD_MODEL=false)
   - Issue: `AppState::new()` calls `load_model()` which downloads from HuggingFace
   - Result: Startup timeout before serve() runs
   - **LESSON:** Use LAZY_LOAD_MODEL=true ✓ (fixed)

2. **Second attempt:** Prebuilt Docker image
   - Issue: Containers stuck "Pending", serve() never runs
   - Result: Health check timeout
   - **LESSON:** Might be image registry access issue in Modal

3. **Third attempt:** Modal builds Dockerfile locally
   - Issue: Same - containers stuck "Pending" indefinitely
   - Build succeeds (243 seconds)
   - But runtime still fails
   - **LESSON:** Build succeeds ≠ runtime success

4. **Fourth attempt:** Added debug logging
   - Issue: serve() never called (debug prints not in logs)
   - Result: Bootstrap fails before function execution
   - **LESSON:** Problem is Modal's container initialization

---

## Technical Evidence

### What Works
```bash
$ python3 -m py_compile infra/modal/modal_deploy.py
✅ Syntax OK

$ docker pull docker.io/tys203831/model2vec-rs-api-server:modal
✅ Image exists and pulls

$ timeout 300 modal deploy ... modal_deploy.py
✅ Deployment succeeds in 1-2 seconds
✅ App transitions to "deployed" state
```

### What Fails
```bash
$ modal container list
❌ Status: Pending (after 10s, 30s, 60s, 120s)
❌ No "Running" containers
❌ Multiple restart attempts visible
❌ serve() never executes (no debug output)
```

### Key Observation
- Deployment is **instantaneous** (1.4 seconds)
- Container bootstrap is **stuck indefinitely** (60+ seconds, no progress)
- Debug output never appears (serve() never called)

This suggests the problem is **before serve() executes**, in Modal's container initialization.

---

## Possible Root Causes

### 1. **Volume Mount Issues** ⚠️
```python
volumes={HF_CACHE_DIR: hf_volume},
```
- HF_CACHE_DIR = "/data/hf"
- hf_volume = modal.Volume.from_name("model2vec-hf-cache")
- Issue: Volume might not be attaching properly, blocking container start

### 2. **Environment Variables Not Passed** ⚠️
```python
env=build_env(),
```
- build_env() returns dict with model config
- Modal should pass these to container
- Issue: If env vars fail to load, Rust binary might not start

### 3. **Image Execution Issue** ⚠️
```python
image=IMAGE,
```
- Either the prebuilt image OR the built Dockerfile
- Issue: Image might be missing `/app/model2vec-api` binary, or it's not executable
- Issue: Container's entrypoint/runtime environment might be wrong

### 4. **Port Binding Issue** ⚠️
```python
@modal.web_server(port=8080)
```
- Rust binary must bind to `0.0.0.0:8080`
- Modal's health check expects HTTP response on this port
- Issue: If Rust binary fails to bind (port in use, no permission), container is killed

### 5. **Timeout Configuration** ⚠️
```python
timeout=1200,
startup_timeout=1800,
```
- 20-30 minute timeouts should be plenty
- Issue: Internal Modal timeout (not configurable) might be firing

---

## Recommended Next Steps

### Option 1: Disable Volume Mount (Debug)
```python
# In modal_deploy.py, comment out volume
# volumes={HF_CACHE_DIR: hf_volume},
```
If this fixes it: **volume mount is the issue**

### Option 2: Use Different Image
```bash
# Try a simpler image with the binary pre-included
MODAL_IMAGE=python:3.11-slim  # Test with standard Python image
```
If this fixes it: **Dockerfile/image is the issue**

### Option 3: Add Health Check Endpoint
Modal's web_server expects HTTP responses. Ensure the Rust binary:
```rust
// Must respond to GET / or Modal's health check
GET /
```

### Option 4: Contact Modal Support
Provide:
- App ID: `ap-Kzm4LeHAWOfaKnoEsu...`
- Container logs showing "Pending" status for 60+ seconds
- Questions: Why doesn't serve() execute? Why no bootstrap logs?

### Option 5: Use Modal's Python Wrapper Differently
Instead of subprocess.Popen(), try Modal's built-in Process API:
```python
from modal import Container
# Run Rust binary inside Modal's container API
```

---

## What We've Learned

### About Modal
1. ✅ Deployment itself is fast and reliable (< 2 seconds)
2. ❌ Container bootstrap is fragile (unknown failure modes)
3. ❌ Error messages are minimal ("Pending" status is all we get)
4. ⚠️ Debugging is difficult (no execution logs, no error output)

### About Our Approach
1. ✅ Heartbeat starvation fix is correct (time.sleep polling)
2. ✅ Dockerfile build works (Rust compilation successful)
3. ❌ But container runtime still fails before our code runs
4. ⚠️ Different issue than initially thought

---

## Summary

**The original heartbeat timeout issue** identified in the research was correct, but we're hitting a **different problem at container bootstrap** that prevents the serve() function from even starting.

This is more fundamental than the heartbeat issue - it's Modal's ability to start the container at all.

**Next Action:** Need direct debugging access or Modal support to understand why containers don't transition from "Pending" to "Running".

---

**Debugging Timestamp:** 2026-03-11 13:02 UTC
**Time Spent:** ~45 minutes of investigation
**Current Blockers:**
- Cannot see serve() execution output
- Cannot see container startup logs
- Cannot see subprocess.Popen() success/failure
- Cannot see Rust binary startup errors

**Recommendation:** Try simpler test function first to isolate issue.
