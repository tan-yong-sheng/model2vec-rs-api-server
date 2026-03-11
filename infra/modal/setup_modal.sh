#!/bin/bash

set -e

ENV_FILE="${ENV_FILE:-infra/modal/.env.modal}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

print_info() {
  echo "[modal] $1"
}

if ! command -v modal >/dev/null 2>&1; then
  print_info "Installing Modal CLI"
  pip install modal
fi

if [ ! -f "${ENV_FILE}" ]; then
  print_info "Creating ${ENV_FILE} from example"
  cp "${ROOT_DIR}/infra/modal/.env.modal.example" "${ENV_FILE}"
fi

print_info "Authenticating (if needed)"
modal token new || true

print_info "Deploying"
ENV_FILE="${ENV_FILE}" modal deploy "${ROOT_DIR}/infra/modal/modal_deploy.py"
