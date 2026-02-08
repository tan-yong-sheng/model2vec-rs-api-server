#!/bin/bash
set -e

# Test multiple models for PR #1 benchmark
# Measures memory usage and reload times across different model sizes

ALIAS_MODEL_NAME="test-model"
PORT="8081"

test_model() {
    local model_name=$1
    local model_size=$2
    local timeout=30

    echo ""
    echo "========================================"
    echo "Testing: $model_name"
    echo "Expected Size: $model_size"
    echo "========================================"

    # Clean up
    docker stop model2vec-test 2>/dev/null || true
    docker rm model2vec-test 2>/dev/null || true

    # Start container with unloading enabled
    echo "Starting container..."
    docker run -d --name model2vec-test \
        -p "$PORT:8080" \
        -e "MODEL_NAME=$model_name" \
        -e "ALIAS_MODEL_NAME=$ALIAS_MODEL_NAME" \
        -e "LAZY_LOAD_MODEL=false" \
        -e "MODEL_UNLOAD_ENABLED=true" \
        -e "MODEL_UNLOAD_IDLE_TIMEOUT=$timeout" \
        -e "RUST_LOG=info" \
        model2vec-benchmark 2>&1

    # Wait for startup
    echo -n "Waiting for startup..."
    local startup_start=$(date +%s)
    local ready=0
    for i in {1..180}; do
        if curl -s -o /dev/null -w "%{http_code}" http://localhost:$PORT/.well-known/ready 2>/dev/null | grep -q "204"; then
            ready=1
            break
        fi
        sleep 1
        echo -n "."
    done
    local startup_end=$(date +%s)

    if [ $ready -eq 0 ]; then
        echo " TIMEOUT - Container logs:"
        docker logs model2vec-test 2>&1 | tail -20
        docker stop model2vec-test 2>/dev/null || true
        docker rm model2vec-test 2>/dev/null || true
        return 1
    fi
    echo " OK ($((startup_end - startup_start))s)"

    # Memory at startup
    sleep 2
    local mem_startup=$(docker stats model2vec-test --no-stream --format "{{.MemUsage}}" 2>/dev/null | cut -d'/' -f1 | tr -d ' ' || echo "N/A")
    echo "Memory at startup: $mem_startup"

    # First request
    echo "Making first request..."
    local first_req_start=$(date +%s%N)
    local response=$(curl -s -X POST "http://localhost:$PORT/v1/embeddings" \
        -H "Content-Type: application/json" \
        -d '{"input": "Hello world test", "model": "test-model"}' \
        -w "\nHTTP_CODE:%{http_code}")
    local first_req_end=$(date +%s%N)
    local http_code=$(echo "$response" | grep "HTTP_CODE:" | cut -d':' -f2)

    if [ "$http_code" != "200" ]; then
        echo "ERROR: First request failed (HTTP $http_code)"
        docker logs model2vec-test 2>&1 | tail -10
        docker stop model2vec-test 2>/dev/null || true
        docker rm model2vec-test 2>/dev/null || true
        return 1
    fi

    local first_req_ms=$(( (first_req_end - first_req_start) / 1000000 ))
    echo "First request latency: ${first_req_ms}ms"

    # Memory after first request
    sleep 2
    local mem_active=$(docker stats model2vec-test --no-stream --format "{{.MemUsage}}" 2>/dev/null | cut -d'/' -f1 | tr -d ' ' || echo "N/A")
    echo "Memory during active use: $mem_active"

    # Wait for idle timeout
    echo "Waiting ${timeout}s for idle timeout..."
    sleep $timeout
    echo "Waiting additional 5s for unload..."
    sleep 5

    # Memory after idle
    local mem_idle=$(docker stats model2vec-test --no-stream --format "{{.MemUsage}}" 2>/dev/null | cut -d'/' -f1 | tr -d ' ' || echo "N/A")
    echo "Memory after idle (unloaded): $mem_idle"

    # CRITICAL: Test reload time
    echo "Testing reload time (first request after unload)..."
    local reload_start=$(date +%s%N)
    local reload_response=$(curl -s -X POST "http://localhost:$PORT/v1/embeddings" \
        -H "Content-Type: application/json" \
        -d '{"input": "Reload test", "model": "test-model"}' \
        -w "\nHTTP_CODE:%{http_code}")
    local reload_end=$(date +%s%N)
    local reload_http=$(echo "$reload_response" | grep "HTTP_CODE:" | cut -d':' -f2)

    if [ "$reload_http" != "200" ]; then
        echo "ERROR: Reload request failed (HTTP $reload_http)"
        reload_time_ms="ERROR"
    else
        local reload_time_ms=$(( (reload_end - reload_start) / 1000000 ))
    fi

    echo "RELOAD TIME: ${reload_time_ms}ms"

    # Memory after reload
    sleep 2
    local mem_after_reload=$(docker stats model2vec-test --no-stream --format "{{.MemUsage}}" 2>/dev/null | cut -d'/' -f1 | tr -d ' ' || echo "N/A")
    echo "Memory after reload: $mem_after_reload"

    # Get logs to verify unload happened
    echo "Container logs (last 5 lines):"
    docker logs model2vec-test 2>&1 | grep -E "(Unloading|unloaded|idle)" | tail -5

    # Cleanup
    docker stop model2vec-test 2>/dev/null || true
    docker rm model2vec-test 2>/dev/null || true

    # Output summary line for easy parsing
    echo ""
    echo "RESULTS_SUMMARY|$model_name|$model_size|$mem_startup|$mem_active|$mem_idle|$reload_time_ms|$mem_after_reload"
    echo ""

    return 0
}

# Main execution
echo "================================================"
echo "Multi-Model Benchmark for PR #1"
echo "================================================"
echo ""
echo "This will test each model sequentially."
echo "Total estimated time: 15-25 minutes"
echo ""

# Models to test
models=(
    "minishlab/potion-base-2M:~2MB"
    "minishlab/potion-base-4M:~4MB"
    "minishlab/potion-base-8M:~8MB"
    "minishlab/potion-base-32M:~32MB"
    "minishlab/potion-retrieval-32M:~32MB"
    "minishlab/potion-multilingual-128M:~128MB"
)

results_file="model_benchmark_results.txt"
echo "Multi-Model Benchmark Results - $(date)" > $results_file
echo "========================================" >> $results_file
echo "" >> $results_file

for model_info in "${models[@]}"; do
    IFS=':' read -r model_name model_size <<< "$model_info"

    if test_model "$model_name" "$model_size"; then
        echo "✅ $model_name completed successfully" >> $results_file
    else
        echo "❌ $model_name FAILED" >> $results_file
    fi
    echo "" >> $results_file
done

echo ""
echo "========================================"
echo "All tests complete!"
echo "Results saved to: $results_file"
echo "========================================"
