# Async Polling Deployment Guide

**Status:** Implementation Complete ✅
**Date:** 2026-03-11
**Version:** 1.0
**Phase:** Phase 2-3 of Modal Optimization Plan

---

## Overview

This document describes the deployment and testing procedure for the **async polling fix** to Modal's heartbeat timeout issue.

### What Changed

**File:** `infra/modal/modal_deploy.py`

**Key Changes:**
1. Added `import asyncio` (line 12)
2. Changed function signature from `def serve()` to `async def serve()` (line 145)
3. Changed blocking sleep from `time.sleep(0.5)` to `await asyncio.sleep(0.5)` (line 195)

**Impact:**
- Modal's event loop can now run `Modal's heartbeat mechanism during the 500ms polling sleep cycles
- Prevents "Runner terminated" crashes caused by heartbeat timeouts
- **No changes to Rust binary, API logic, or configuration schema**

### Why This Fix Works

Modal's heartbeat loop needs to send periodic health checks to the host. When the Python code was blocking with `time.sleep()`, the event loop couldn't process these checks properly. By switching to `await asyncio.sleep()`, we yield control to the event loop, allowing:

1. Modal heartbeat to send health pulses
2. Health check responses to be processed
3. HTTP requests to be handled
4. Container to stay alive and healthy

---

## Pre-Deployment Checklist

- [x] Code changes completed
- [x] Python syntax validated
- [x] `asyncio` module imported correctly
- [x] Function signature changed to `async`
- [x] `time.sleep()` replaced with `await asyncio.sleep()`
- [x] Comments updated to reflect async mode
- [ ] Local docker-compose test (recommended before Modal deploy)
- [ ] Reviewed Modal environment configuration

---

## Deployment Steps

### Step 1: Local Verification (Optional but Recommended)

Test that the Rust binary still works locally:

```bash
# Navigate to project root
cd /workspaces/model2vec-rs-api-server

# Test with docker-compose
docker compose -f infra/modal/docker-compose.yml down
docker compose -f infra/modal/docker-compose.yml up -d

# Wait 5 seconds for startup
sleep 5

# Test endpoints
echo "Testing health checks..."
curl -s -w "\nStatus: %{http_code}\n" http://localhost:8080/.well-known/live
curl -s -w "\nStatus: %{http_code}\n" http://localhost:8080/.well-known/ready

echo "Testing model list..."
curl -s -w "\nStatus: %{http_code}\n" http://localhost:8080/v1/models | jq .

# Cleanup
docker compose -f infra/modal/docker-compose.yml down
```

**Expected Output:**
- Health checks return 204 (No Content)
- Model list returns 200 with JSON payload
- Timestamps on requests show fast response times

### Step 2: Deploy to Modal

```bash
# Set environment variables for deployment
cd /workspaces/model2vec-rs-api-server

# Deploy with custom .env.modal file
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
```

**Expected Output:**
```
✓ Created app 'model2vec-api'
✓ Pushing image 'model2vec-api' (...)
✓ Created function 'serve' (...)
✓ Deployed app successfully
App deployed at: https://...--model2vec-api-serve.modal.run
```

**⏱️ Wait Time:** 2-3 minutes for deployment to complete

### Step 3: Capture Container ID

```bash
# List containers and find the model2vec-api function
modal container list --json | jq '.[] | select(.function_name == "serve")' | head -20

# Extract container ID for later use
CONTAINER_ID=$(modal container list --json | jq -r '.[] | select(.function_name == "serve") | .id' | head -1)
echo "Container ID: $CONTAINER_ID"
```

---

## Testing Procedure

### Test A: Health Checks (Immediate)

Run these checks immediately after deployment:

```bash
# Get the deployed URL from modal app list
modal app list

# Test live health check (should be fast)
curl -s -w "\nStatus: %{http_code}, Time: %{time_total}s\n" https://<your-url>/. well-known/live

# Expected: 204 status in <1 second
```

### Test B: Container Logs Inspection (First 2 Minutes)

Monitor logs for success indicators or errors:

```bash
# Get real-time logs
CONTAINER_ID=$(modal container list --json | jq -r '.[0].id')
modal container logs $CONTAINER_ID --follow

# Look for:
# ✅ "🚀 serve() STARTED (ASYNC MODE)"
# ✅ "✅ Subprocess started with PID: ..."
# ✅ "⏱️  Polling... (0.5s elapsed, PID ... still running)"
# ❌ NO "Runner terminated" message
# ❌ NO "heartbeat timeout" message
```

**Success Indicators:**
```
================================================================================
🚀 serve() STARTED (ASYNC MODE)
================================================================================
📝 Environment variables set: X total
   PORT=8080
   MODEL_NAME=minishlab/potion-multilingual-128M
   LAZY_LOAD_MODEL=true
   RUST_LOG=info

🔨 Starting subprocess: /app/model2vec-api
   Working directory: /app
   Binary exists: True

✅ Subprocess started with PID: 42

⏱️  Polling... (0.5s elapsed, PID 42 still running)
⏱️  Polling... (1.0s elapsed, PID 42 still running)
...
[Rust server logs...]
...
```

### Test C: API Endpoint Test (2-5 Minutes After Deploy)

Test that the embeddings API works:

```bash
# Replace with your actual Modal URL from "modal app list"
MODAL_URL="https://your-username--model2vec-api-serve.modal.run"

# Test embeddings (will trigger lazy model load if enabled)
curl -X POST "$MODAL_URL/v1/embeddings" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "minishlab/potion-multilingual-128M",
    "input": "hello world"
  }' \
  -w "\nStatus: %{http_code}, Time: %{time_total}s\n" \
  -v
```

**Expected Behavior:**
- **First request:** Takes 30-180 seconds (model loads from HuggingFace)
- **Subsequent requests:** Return in <2 seconds (model cached in memory)
- **Response:** 200 status with embeddings in JSON format

### Test D: Sustained Stability (5-10 Minutes)

Monitor the container to ensure it stays alive:

```bash
# Watch logs for full 5+ minutes (check for no "Runner terminated")
modal container logs $CONTAINER_ID --follow

# In another terminal, send periodic requests every 30 seconds
for i in {1..10}; do
  echo "Request $i at $(date)"
  curl -s "$MODAL_URL/.well-known/live" -w "Status: %{http_code}\n"
  sleep 30
done

# Expected: All requests return 204, no container restarts
```

### Test E: Container Scaling (Optional Advanced Test)

If auto-scaling is enabled:

```bash
# Send concurrent requests to trigger scale-up
for i in {1..5}; do
  curl -X POST "$MODAL_URL/v1/embeddings" \
    -H "Content-Type: application/json" \
    -d '{"input": "test"}' \
    -w "\nRequest $i: %{http_code}\n" &
done

wait

# Monitor logs for new containers
modal container list
modal container logs --function serve --follow
```

---

## Success Criteria

✅ **All criteria must be met for successful deployment:**

| Criterion | Status | Notes |
|-----------|--------|-------|
| Container stays alive >5 min | [ ] | No "Runner terminated" messages |
| No heartbeat timeout errors | [ ] | Check logs for "heartbeat timeout" string |
| Health checks return 204 | [ ] | `/.well-known/live` responds in <1s |
| Health checks return 204 | [ ] | `/.well-known/ready` responds in <1s |
| Model list returns 200 | [ ] | `/v1/models` returns JSON array |
| Embeddings endpoint works | [ ] | First request takes 30-180s, subsequent <2s |
| Server initializes with "ASYNC MODE" message | [ ] | Check logs for "STARTED (ASYNC MODE)" |
| Polling messages appear in logs | [ ] | Check for "⏱️ Polling..." messages |
| No crashes or exceptions | [ ] | Search logs for 💥 emoji and exception messages |
| Concurrent requests handled | [ ] | Multiple simultaneous requests return successfully |

---

## Rollback Plan

If the async polling fix causes issues:

### Quick Rollback (5 minutes)

```bash
# Revert to previous version (blocking sleep)
git checkout infra/modal/modal_deploy.py

# Redeploy immediately
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py

# Verify rollback
modal container list
modal container logs <container-id> --tail 50
```

### Root Cause Analysis If Needed

If rollback is necessary, check for:
1. **Syntax errors:** Python version incompatibility with `async/await`
2. **Modal version incompatibility:** Older Modal versions may not support async web_server
3. **Event loop conflicts:** Another library might conflict with asyncio usage

---

## Monitoring After Successful Deployment

### Daily Checks (Automated)

Set up recurring health checks:

```bash
# Create a cron job to monitor deployment
# Every 6 hours, check if container is still healthy
*/6 * * * * curl -s https://<your-url>/.well-known/live && echo "OK at $(date)" >> /tmp/modal_health.log

# Monitor logs daily
0 9 * * * modal container logs --function serve | tail -100 > /tmp/modal_daily.log
```

### Performance Metrics

Collect these baseline metrics after deployment:

```bash
# Measure response times over 1 hour
for i in {1..120}; do
  echo "Sample $i at $(date +%s)"
  time curl -s https://<your-url>/.well-known/live > /dev/null
  sleep 30  # Every 30 seconds
done | tee /tmp/response_times.log
```

---

## Troubleshooting

### Issue: "Runner terminated" still appears

**Cause:** Modal's heartbeat timeout is still occurring

**Solutions:**
1. **Increase memory:** Try `MODAL_MEMORY_MB=3072` (was 2048)
2. **Increase CPU:** Try `MODAL_CPU=0.5` (was 0.25)
3. **Check Rust logs:** Look for crashes or panics in subprocess output
4. **Escalate to Phase 4:** Implement lifecycle hooks if async polling insufficient

### Issue: Async function not supported

**Cause:** Modal version too old, or async not supported on web_server

**Solution:**
```bash
# Check Modal version
modal --version

# Upgrade if needed
pip install --upgrade modal

# Verify async web_server is supported (Modal 0.13+)
```

### Issue: Container takes >30 seconds to respond to health checks

**Cause:** Model loading is blocking the event loop during startup

**Solution:**
1. Reduce model size (use potion-base-8M instead of larger models)
2. Increase startup_timeout: `MODAL_STARTUP_TIMEOUT_SECS=3600` (1 hour)
3. Enable lazy loading: `LAZY_LOAD_MODEL=true`

### Issue: Subprocess crashes silently

**Cause:** Rust binary encountering an error

**Solution:**
```bash
# Check subprocess output in logs
modal container logs <id> --follow | grep -A 5 "Process exited"

# Test Rust binary locally
docker run --rm model2vec-rs-api:latest /app/model2vec-api --help

# Check environment variables are correct
modal container logs <id> | grep "Environment variables"
```

---

## Next Steps

1. ✅ **Deploy** using Step 2 above
2. ✅ **Test** using Test A-E above
3. ✅ **Verify** all success criteria met
4. ✅ **Monitor** with daily health checks
5. 📝 **Document** any issues encountered
6. 🚀 **Announce** successful deployment to team

---

## Reference Links

- **Modal Docs:** https://modal.com/docs/guide/apps
- **Modal Async Support:** https://modal.com/docs/guide/async
- **Deployment Logs:** Use `modal container logs <id>`
- **Troubleshooting:** https://modal.com/docs/guide/troubleshooting

---

## Contact & Escalation

If deployment fails after following all steps:

1. Collect logs: `modal container logs <id> > /tmp/logs.txt`
2. Check research docs: `infra/modal/research/`
3. Consider Phase 4: Lifecycle hooks approach
4. Escalate to Modal support with logs and steps taken

---

**Last Updated:** 2026-03-11
**Author:** Claude Code
**Status:** Ready for Deployment ✅
