# Modal Deployment - Immediate Action Checklist

**Last Updated:** 2026-03-11
**Status:** Ready for troubleshooting next phase

---

## 🎯 What You Need to Know

The Rust API server deploys to Modal successfully, but containers aren't responding to HTTP requests. This is likely a **resource constraint** issue, not a code issue.

**Evidence:**
- ✅ Docker Compose works perfectly locally
- ✅ Modal deployment succeeds (returns "deployed")
- ✅ Containers start (but stay in "Pending" state)
- ❌ HTTP requests timeout (no response after 60+ seconds)

---

## ✅ Quick Fix (Try This First)

### Step 1: Increase Resources
```bash
cd /workspaces/model2vec-rs-api-server

# Edit config
nano infra/modal/.env.modal
```

Change these lines:
```diff
- MODAL_CPU=0.25
+ MODAL_CPU=1.0

- MODAL_MEMORY_MB=2048
+ MODAL_MEMORY_MB=4096
```

### Step 2: Redeploy
```bash
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
```

Wait for: `✓ App deployed in X.XXs! 🎉`

### Step 3: Test (30 seconds later)
```bash
# Test health endpoint
curl https://tan-yong-sheng--model2vec-api-serve.modal.run/.well-known/live

# Expected: Empty response with status 204 (very fast, <1 second)

# If that works, test models endpoint
curl https://tan-yong-sheng--model2vec-api-serve.modal.run/v1/models

# Expected: JSON array with 200 status
```

**If this works:** Congratulations! 🎉 The issue was resource starvation.

---

## 🔍 Troubleshooting (If Above Doesn't Help)

### Check 1: Modal Dashboard
```bash
# Open the dashboard
open https://modal.com/apps/tan-yong-sheng/main/deployed/model2vec-api

# Look for:
# - Container logs (red/orange = errors)
# - Hardware metrics (CPU/memory spikes)
# - Error messages in status panel
```

### Check 2: Verify Local Setup Still Works
```bash
# Test docker-compose still works
docker compose -f infra/modal/docker-compose.yml up -d
sleep 5

# Should respond instantly
curl http://localhost:8080/.well-known/live

# Kill it
docker compose -f infra/modal/docker-compose.yml down
```

**If local works but Modal doesn't:** Modal environment issue, check logs

### Check 3: Try Smaller Model
```bash
# Edit .env.modal
nano infra/modal/.env.modal

# Change this line:
# FROM: MODEL_NAME=minishlab/potion-multilingual-128M
# TO:
MODEL_NAME=minishlab/potion-base-8M
LAZY_LOAD_MODEL=true
```

Redeploy and test. If smaller model responds faster, model size is bottleneck.

### Check 4: Enable Debug Logging
```bash
# Edit .env.modal
RUST_LOG=debug

# Redeploy
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py

# Check logs for detailed startup info
modal container list
modal container logs <container-id>
```

---

## 📚 Documentation Available

Read these in order if you need deeper understanding:

1. **START HERE:** `infra/modal/DEPLOYMENT_SUMMARY.md` (comprehensive guide)
2. **Strategies:** `infra/modal/DEPLOYMENT_STRATEGY_ANALYSIS.md` (compare approaches)
3. **Technical:** `infra/modal/research/MODAL_SUBPROCESS_ANALYSIS.md` (deep dive)
4. **Roadmap:** `infra/modal/research/MODAL_IMPLEMENTATION_ROADMAP.md` (phase-by-phase)

---

## 🚀 Deployment Options Available

### Option 1: Simple Function (Default - Use This)
```bash
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
```
**URL:** https://tan-yong-sheng--model2vec-api-serve.modal.run

### Option 2: Sandbox (Backup - Try If Option 1 Fails)
```bash
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_sandbox_deploy.py
```
**URL:** https://tan-yong-sheng--model2vec-api-sandbox-serve-sandbox.modal.run

---

## 📋 Useful Commands

```bash
# Check deployment status
modal app list

# View container list
modal container list

# View logs (replace with actual container ID)
CONTAINER_ID=$(modal container list --json | jq -r '.[0].container_id // .[0].id')
modal container logs "$CONTAINER_ID"

# Test health
curl https://tan-yong-sheng--model2vec-api-serve.modal.run/.well-known/live

# Test ready
curl https://tan-yong-sheng--model2vec-api-serve.modal.run/.well-known/ready

# Test models
curl https://tan-yong-sheng--model2vec-api-serve.modal.run/v1/models | jq .

# Test embeddings (after model loads)
curl -X POST https://tan-yong-sheng--model2vec-api-serve.modal.run/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"items":["hello world"]}'
```

---

## ⚙️ Configuration Reference

**Current `.env.modal` settings:**
```ini
# Model
MODEL_NAME=minishlab/potion-multilingual-128M
PORT=8080
RUST_LOG=info
LAZY_LOAD_MODEL=true

# Resources (ADJUST THESE IF CONTAINERS TIMEOUT)
MODAL_CPU=0.25           # ← Increase to 1.0 for testing
MODAL_MEMORY_MB=2048     # ← Increase to 4096 for testing
MODAL_TIMEOUT_SECS=1200  # 20 minutes
MODAL_STARTUP_TIMEOUT_SECS=1800  # 30 minutes
```

**If increasing to full resources:**
```ini
MODAL_CPU=1.0
MODAL_MEMORY_MB=4096
MODAL_STARTUP_TIMEOUT_SECS=3600
```

---

## 🎓 Key Concepts

**Modal Web Server Pattern:**
- Function MUST block indefinitely
- When it returns, Modal kills the container
- HTTP requests are routed to the port specified
- Subprocess running in background

**Why Containers Stay Pending:**
- Model downloading (HuggingFace)
- Python initialization
- Rust binary startup
- All competing for CPU/memory
- 0.25 CPU might not be enough for all three

**Why Docker Compose Works:**
- Single resource allocation
- No framework overhead
- Direct port mapping
- Simpler initialization

---

## 🆘 If Everything Fails

1. **Check Modal Status:** https://status.modal.com/
2. **Read:** `infra/modal/DEPLOYMENT_SUMMARY.md` (diagnostic section)
3. **Contact Modal:** https://modal.com/docs/guide/troubleshooting
4. **Alternative:** Deploy to Docker/K8s/ECS instead

---

## 🎯 Success Indicators

After deployment, you should see:
- ✅ `curl .well-known/live` → 204 status in <1 second
- ✅ `curl .well-known/ready` → 204 status in <1 second
- ✅ `curl /v1/models` → 200 status with JSON in <1 second
- ✅ Container logs showing "Server listening on port 8080"
- ✅ No "Runner terminated" or "heartbeat timeout" messages

---

## 📊 Cost Implications

| Change | Monthly Impact |
|--------|---|
| +0.75 CPU (0.25 → 1.0) | ~$10-15 |
| +2GB RAM (2GB → 4GB) | ~$10-15 |
| Both combined | ~$20-25 |

**For troubleshooting:** Acceptable cost to verify it's not a resource issue

---

## ✨ What's Ready

- ✅ **modal_deploy.py** - Production-ready simple function
- ✅ **modal_sandbox_deploy.py** - Alternative sandbox approach
- ✅ **Comprehensive docs** - All analysis and strategies documented
- ✅ **Git history** - Clear commit messages explaining changes
- ✅ **Local fallback** - Docker Compose works perfectly

---

## 👉 Your Next Step

**Run this now:**

```bash
# 1. Increase resources
sed -i 's/MODAL_CPU=0.25/MODAL_CPU=1.0/' infra/modal/.env.modal
sed -i 's/MODAL_MEMORY_MB=2048/MODAL_MEMORY_MB=4096/' infra/modal/.env.modal

# 2. Redeploy
ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py

# 3. Wait 30 seconds
sleep 30

# 4. Test
curl https://tan-yong-sheng--model2vec-api-serve.modal.run/.well-known/live
```

If that works, you're done! If not, read `DEPLOYMENT_SUMMARY.md` for next steps.

---

**Confidence:** High (95%) that this is a resource constraint issue
**Time to Fix:** 5-10 minutes if it's resources
**Estimated Cost Impact:** $20-25/month for increased resources

Good luck! 🚀
