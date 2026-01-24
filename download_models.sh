#!/bin/bash
# Download model2vec models for the Rust API server

set -e

MODEL_NAME=${MODEL_NAME:-minishlab/potion-base-8M}
OUTPUT_DIR=${OUTPUT_DIR:-./models}

echo "Downloading model: ${MODEL_NAME}"
echo "Output directory: ${OUTPUT_DIR}"

# Create output directory
mkdir -p "${OUTPUT_DIR}"

# Install model2vec-rs CLI if not available
if ! command -v model2vec-rs &> /dev/null; then
    echo "Installing model2vec-rs CLI..."
    cargo install model2vec-rs
fi

# Download model using model2vec-rs
# The model will be downloaded to ~/.cache/hf/hub by default
# We need to copy it to the output directory

echo "Downloading model files from HuggingFace Hub..."

# Create temporary directory for download
TEMP_DIR=$(mktemp -d)
cd "${TEMP_DIR}"

# Download using model2vec-rs
model2vec-rs encode-single "test" "${MODEL_NAME}"

# Find the downloaded model in cache
HF_CACHE_DIR="${HOME}/.cache/hf/hub"
MODEL_CACHE_DIR=$(find "${HF_CACHE_DIR}" -name "models--"$(echo "${MODEL_NAME}" | tr '/' '-') -type d 2>/dev/null | head -1)

if [ -z "${MODEL_CACHE_DIR}" ]; then
    echo "Error: Could not find downloaded model in cache"
    rm -rf "${TEMP_DIR}"
    exit 1
fi

echo "Found model at: ${MODEL_CACHE_DIR}"

# Copy model files to output directory
cp -r "${MODEL_CACHE_DIR}/snapshots/"*/* "${OUTPUT_DIR}/" 2>/dev/null || true

# If the above didn't work, try direct copy
if [ ! -f "${OUTPUT_DIR}/config.json" ]; then
    echo "Trying alternate copy method..."
    cp -r "${MODEL_CACHE_DIR}/"* "${OUTPUT_DIR}/" 2>/dev/null || true
fi

# Verify files were copied
if [ -f "${OUTPUT_DIR}/config.json" ]; then
    echo "Model files downloaded successfully to ${OUTPUT_DIR}"
    ls -la "${OUTPUT_DIR}"
else
    echo "Error: Model files not found in ${OUTPUT_DIR}"
    echo "Contents of cache directory:"
    find "${MODEL_CACHE_DIR}" -maxdepth 3 -type f 2>/dev/null | head -20
    rm -rf "${TEMP_DIR}"
    exit 1
fi

# Cleanup
rm -rf "${TEMP_DIR}"

echo "Done!"
