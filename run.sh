#!/bin/bash
# Build and run the Model2Vec Rust API server

set -e

echo "=== Model2Vec Rust API Server ==="
echo ""

# Check if models directory exists
if [ ! -d "./models" ]; then
    echo "Models directory not found. Downloading model..."

    # Create models directory
    mkdir -p ./models

    # Download model using HuggingFace CLI or direct download
    MODEL_NAME=${MODEL_NAME:-minishlab/potion-base-8M}

    echo "Downloading ${MODEL_NAME}..."

    # Try using huggingface_hub Python library if available
    if command -v python3 &> /dev/null; then
        python3 -c "
from huggingface_hub import hf_hub_download
import os

model_name = os.environ.get('MODEL_NAME', 'minishlab/potion-base-8M')
output_dir = './models'

# Download required files
files = ['config.json', 'model.safetensors', 'tokenizer.json']
for f in files:
    try:
        path = hf_hub_download(repo_id=model_name, filename=f, repo_type='model')
        import shutil
        shutil.copy(path, os.path.join(output_dir, f))
        print(f'Downloaded: {f}')
    except Exception as e:
        print(f'Error downloading {f}: {e}')
        raise
"
    else
        echo "Python3 not found. Please install huggingface_hub or download models manually:"
        echo "  - config.json"
        echo "  - model.safetensors"
        echo "  - tokenizer.json"
        echo ""
        echo "Place these files in the ./models directory."
    fi
else
    echo "Models directory found."
fi

# List model files
echo ""
echo "Model files:"
ls -la ./models/ 2>/dev/null || echo "No files found"

# Build and run with Docker
echo ""
echo "Building Docker image..."
docker compose build

echo ""
echo "Starting container..."
docker compose up -d

echo ""
echo "=== Server started ==="
echo "API available at: http://localhost:8080"
echo ""
echo "Endpoints:"
echo "  - Health: curl http://localhost:8080/.well-known/ready"
echo "  - Models: curl http://localhost:8080/v1/models"
echo "  - Embeddings: curl -X POST http://localhost:8080/v1/embeddings -H 'Content-Type: application/json' -d '{\"input\": \"hello world\", \"model\": \"minishlab/potion-base-8M\"}'"
