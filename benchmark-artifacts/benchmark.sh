#!/bin/bash
set -e

# Benchmark script for model2vec-rs-api-server PR #1
# Tests lazy loading and model unloading performance

MODEL_NAME="${MODEL_NAME:-minishlab/potion-base-8M}"
ALIAS_MODEL_NAME="${ALIAS_MODEL_NAME:-test-model}"
PORT="${PORT:-8080}"

echo "=================================="
echo "Model2Vec API Server Benchmark"
echo "Model: $MODEL_NAME"
echo "=================================="

# Function to start container with specific settings
start_container() {
    local name=$1
    local lazy_load=$2
    local unload_enabled=$3
    local timeout=$4

    echo ""
    echo "--- Starting container: $name ---"
    echo "LAZY_LOAD_MODEL=$lazy_load"
    echo "MODEL_UNLOAD_ENABLED=$unload_enabled"
    echo "MODEL_UNLOAD_IDLE_TIMEOUT=${timeout}s"

    docker run -d --name "$name" \
        -p "$PORT:8080" \
        -e "MODEL_NAME=$MODEL_NAME" \
        -e "ALIAS_MODEL_NAME=$ALIAS_MODEL_NAME" \
        -e "LAZY_LOAD_MODEL=$lazy_load" \
        -e "MODEL_UNLOAD_ENABLED=$unload_enabled" \
        -e "MODEL_UNLOAD_IDLE_TIMEOUT=$timeout" \
        model2vec-benchmark
}

# Function to wait for server to be ready
wait_for_ready() {
    local max_wait=${1:-120}
    local waited=0
    echo -n "Waiting for server to be ready..."
    while ! curl -s -o /dev/null -w "%{http_code}" http://localhost:$PORT/.well-known/ready | grep -q "204"; do
        sleep 1
        waited=$((waited + 1))
        if [ $waited -ge $max_wait ]; then
            echo " TIMEOUT"
            return 1
        fi
        echo -n "."
    done
    echo " OK (${waited}s)"
}

# Function to get memory usage
get_memory() {
    docker stats "$1" --no-stream --format "{{.MemUsage}}" 2>/dev/null | cut -d'/' -f1 | tr -d ' ' || echo "N/A"
}

# Function to make embedding request
test_request() {
    local label=$1
    echo -n "$label: "
    local start=$(date +%s%N)
    local response=$(curl -s -X POST "http://localhost:$PORT/v1/embeddings" \
        -H "Content-Type: application/json" \
        -d '{"input": "Hello world this is a test sentence", "model": "test-model"}' \
        -w "\nHTTP_CODE:%{http_code}\nTIME_TOTAL:%{time_total}")
    local end=$(date +%s%N)
    local http_code=$(echo "$response" | grep "HTTP_CODE:" | cut -d':' -f2)
    local time_total=$(echo "$response" | grep "TIME_TOTAL:" | cut -d':' -f2)

    if [ "$http_code" = "200" ]; then
        echo "SUCCESS (total: ${time_total}s)"
        return 0
    else
        echo "FAILED (HTTP $http_code)"
        return 1
    fi
}

# Function to run full benchmark on a container
benchmark_scenario() {
    local scenario_name=$1
    local lazy_load=$2
    local unload_enabled=$3
    local timeout=$4

    echo ""
    echo "=================================="
    echo "Scenario: $scenario_name"
    echo "=================================="

    # Clean up any existing container
    docker stop model2vec-test 2>/dev/null || true
    docker rm model2vec-test 2>/dev/null || true

    # Start timing startup
    local startup_start=$(date +%s)
    start_container "model2vec-test" "$lazy_load" "$unload_enabled" "$timeout"

    # Wait for server ready (returns immediately regardless of model load)
    wait_for_ready
    local startup_end=$(date +%s)
    local startup_time=$((startup_end - startup_start))

    # Memory at startup
    sleep 2
    local mem_startup=$(get_memory "model2vec-test")
    echo "Startup time: ${startup_time}s"
    echo "Memory at startup: $mem_startup"

    # First request (triggers lazy load if enabled)
    echo ""
    echo "Testing first request..."
    local first_req_start=$(date +%s)
    test_request "First request"
    local first_req_end=$(date +%s)
    local first_req_time=$((first_req_end - first_req_start))
    echo "First request total time: ${first_req_time}s"

    # Memory after first request
    sleep 2
    local mem_after_first=$(get_memory "model2vec-test")
    echo "Memory after first request: $mem_after_first"

    # Warm up requests
    echo ""
    echo "Running warm-up requests..."
    for i in 1 2 3; do
        test_request "Request $i"
        sleep 1
    done

    # Memory after active use
    sleep 2
    local mem_active=$(get_memory "model2vec-test")
    echo "Memory during active use: $mem_active"

    # If unloading is enabled, wait for idle timeout and test reload
    if [ "$unload_enabled" = "true" ]; then
        echo ""
        echo "Waiting ${timeout}s for idle timeout..."
        sleep "$timeout"

        # Wait a bit more for unload to complete
        echo "Waiting additional 5s for unload to complete..."
        sleep 5

        # Memory after idle
        local mem_idle=$(get_memory "model2vec-test")
        echo "Memory after idle: $mem_idle"

        # *** CRITICAL TEST: First request after unload ***
        echo ""
        echo "*** CRITICAL: Testing reload after unload ***"
        local reload_start=$(date +%s)
        test_request "Reload request"
        local reload_end=$(date +%s)
        local reload_time=$((reload_end - reload_start))
        echo "*** RELOAD TIME: ${reload_time}s ***"

        # Memory after reload
        sleep 2
        local mem_after_reload=$(get_memory "model2vec-test")
        echo "Memory after reload: $mem_after_reload"
    else
        # Just wait and check memory stays consistent
        echo ""
        echo "Waiting 30s (simulating idle period)..."
        sleep 30
        local mem_after_wait=$(get_memory "model2vec-test")
        echo "Memory after idle period: $mem_after_wait"
    fi

    # Cleanup
    docker stop model2vec-test 2>/dev/null || true
    docker rm model2vec-test 2>/dev/null || true

    # Summary
    echo ""
    echo "--- SUMMARY: $scenario_name ---"
    echo "Startup time: ${startup_time}s"
    echo "Memory startup: $mem_startup"
    if [ "$lazy_load" = "true" ]; then
        echo "First request time: ${first_req_time}s (includes model load)"
    fi
    echo "Memory active: $mem_active"
    if [ "$unload_enabled" = "true" ]; then
        echo "Memory idle: $mem_idle"
        echo "RELOAD TIME: ${reload_time}s"
        echo "Memory after reload: $mem_after_reload"
    fi
}

# Run scenarios

echo "Starting comprehensive benchmark..."
echo "This will take approximately 5-8 minutes"

# Scenario A: Baseline (eager load, no unload)
benchmark_scenario "A: Baseline (Eager, No Unload)" "false" "false" "30"

# Scenario B: Lazy loading only
benchmark_scenario "B: Lazy Loading Only" "true" "false" "30"

# Scenario C: Eager + Unload (THE KEY TEST)
benchmark_scenario "C: Eager + Unload (30s timeout)" "false" "true" "30"

# Scenario D: Lazy + Unload
benchmark_scenario "D: Lazy + Unload (30s timeout)" "true" "true" "30"

echo ""
echo "=================================="
echo "BENCHMARK COMPLETE"
echo "=================================="
