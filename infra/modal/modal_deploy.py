#!/usr/bin/env python3
"""
Modal deployment for model2vec-rs-api-server.

Usage:
  ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py
"""

from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path
from typing import Dict

import modal

ROOT = Path(__file__).resolve().parents[2]
APP_NAME = os.getenv("MODAL_APP_NAME", "model2vec-api")
ENV_FILE = os.getenv("ENV_FILE", str(ROOT / "infra" / "modal" / ".env.modal"))

DEFAULTS = {
    "MODEL_NAME": "minishlab/potion-base-8M",
    "ALIAS_MODEL_NAME": "",
    "PORT": "8080",
    "RUST_LOG": "info",
    "LAZY_LOAD_MODEL": "false",
    "MODEL_UNLOAD_ENABLED": "false",
    "MODEL_UNLOAD_IDLE_TIMEOUT": "1800",
    "EMBEDDING_CACHE_MAX_ENTRIES": "1024",
    "EMBEDDING_CACHE_TTL_SECS": "600",
    "MAX_INPUT_ITEMS": "512",
    "MAX_INPUT_CHARS": "8192",
    "MAX_TOTAL_CHARS": "200000",
    "REQUEST_TIMEOUT_SECS": "30",
    "REQUEST_BODY_LIMIT_BYTES": "2000000",
    "CONCURRENCY_LIMIT": "16",
    "MODEL_LOAD_MAX_RETRIES": "5",
    "MODEL_LOAD_RETRY_BASE_MS": "200",
    "MODEL_LOAD_RETRY_MAX_MS": "5000",
    "MODEL_LOAD_TIMEOUT_SECS": "120",
    "INFERENCE_MAX_RETRIES": "2",
    "INFERENCE_RETRY_BASE_MS": "50",
    "INFERENCE_RETRY_MAX_MS": "500",
    # Modal-specific defaults
    "MODAL_CPU": "0.25",
    "MODAL_MEMORY_MB": "1024",
    "MODAL_TIMEOUT_SECS": "600",
    "MODAL_MIN_CONTAINERS": "0",
    "MODAL_MAX_CONTAINERS": "5",
    "MODAL_SCALEDOWN_WINDOW": "300",
    "MODAL_VOLUME_NAME": "model2vec-hf-cache",
    "MODAL_HF_CACHE_DIR": "/data/hf",
    # Optional: use existing registry image (e.g., ghcr.io/owner/repo:tag)
    "MODAL_IMAGE": "",
}


def load_config(path: str) -> Dict[str, str]:
    config: Dict[str, str] = {}
    if Path(path).exists():
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" in line:
                    key, value = line.split("=", 1)
                    config[key.strip()] = value.strip()
    return config


CONFIG = load_config(ENV_FILE)


def cfg(key: str) -> str:
    return os.getenv(key, CONFIG.get(key, DEFAULTS.get(key, "")))


HF_CACHE_DIR = cfg("MODAL_HF_CACHE_DIR")
VOLUME_NAME = cfg("MODAL_VOLUME_NAME")

def build_image() -> modal.Image:
    image_ref = cfg("MODAL_IMAGE")
    if image_ref:
        return modal.Image.from_registry(image_ref)
    return modal.Image.from_dockerfile(ROOT / "Dockerfile")


IMAGE = build_image()

app = modal.App(name=APP_NAME, image=IMAGE)

hf_volume = modal.Volume.from_name(VOLUME_NAME, create_if_missing=True)


def build_env() -> Dict[str, str]:
    env = {
        "MODEL_NAME": cfg("MODEL_NAME"),
        "ALIAS_MODEL_NAME": cfg("ALIAS_MODEL_NAME"),
        "PORT": cfg("PORT"),
        "RUST_LOG": cfg("RUST_LOG"),
        "LAZY_LOAD_MODEL": cfg("LAZY_LOAD_MODEL"),
        "MODEL_UNLOAD_ENABLED": cfg("MODEL_UNLOAD_ENABLED"),
        "MODEL_UNLOAD_IDLE_TIMEOUT": cfg("MODEL_UNLOAD_IDLE_TIMEOUT"),
        "EMBEDDING_CACHE_MAX_ENTRIES": cfg("EMBEDDING_CACHE_MAX_ENTRIES"),
        "EMBEDDING_CACHE_TTL_SECS": cfg("EMBEDDING_CACHE_TTL_SECS"),
        "MAX_INPUT_ITEMS": cfg("MAX_INPUT_ITEMS"),
        "MAX_INPUT_CHARS": cfg("MAX_INPUT_CHARS"),
        "MAX_TOTAL_CHARS": cfg("MAX_TOTAL_CHARS"),
        "REQUEST_TIMEOUT_SECS": cfg("REQUEST_TIMEOUT_SECS"),
        "REQUEST_BODY_LIMIT_BYTES": cfg("REQUEST_BODY_LIMIT_BYTES"),
        "CONCURRENCY_LIMIT": cfg("CONCURRENCY_LIMIT"),
        "MODEL_LOAD_MAX_RETRIES": cfg("MODEL_LOAD_MAX_RETRIES"),
        "MODEL_LOAD_RETRY_BASE_MS": cfg("MODEL_LOAD_RETRY_BASE_MS"),
        "MODEL_LOAD_RETRY_MAX_MS": cfg("MODEL_LOAD_RETRY_MAX_MS"),
        "MODEL_LOAD_TIMEOUT_SECS": cfg("MODEL_LOAD_TIMEOUT_SECS"),
        "INFERENCE_MAX_RETRIES": cfg("INFERENCE_MAX_RETRIES"),
        "INFERENCE_RETRY_BASE_MS": cfg("INFERENCE_RETRY_BASE_MS"),
        "INFERENCE_RETRY_MAX_MS": cfg("INFERENCE_RETRY_MAX_MS"),
        "HF_HOME": HF_CACHE_DIR,
        "HF_HUB_CACHE": f"{HF_CACHE_DIR}/hub",
    }
    return {k: v for k, v in env.items() if v != ""}


@app.function(
    image=IMAGE,
    cpu=float(cfg("MODAL_CPU")),
    memory=int(cfg("MODAL_MEMORY_MB")),
    timeout=int(cfg("MODAL_TIMEOUT_SECS")),
    min_containers=int(cfg("MODAL_MIN_CONTAINERS")),
    max_containers=int(cfg("MODAL_MAX_CONTAINERS")),
    scaledown_window=int(cfg("MODAL_SCALEDOWN_WINDOW")),
    volumes={HF_CACHE_DIR: hf_volume},
    env=build_env(),
)
@modal.web_server(port=int(cfg("PORT")))
def serve() -> None:
    env = os.environ.copy()
    env.update(build_env())
    subprocess.run(["/app/model2vec-api"], env=env, check=True)


@app.local_entrypoint()
def main() -> None:
    print("Modal app file ready.")
    print("Deploy with:")
    print("  ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Modal deploy wrapper")
    parser.add_argument("--config", type=str, default=ENV_FILE, help="Path to .env file")
    parser.add_argument("--deploy", action="store_true", help="Deploy using Modal CLI")
    args = parser.parse_args()

    os.environ["ENV_FILE"] = args.config
    if args.deploy:
        subprocess.run(["modal", "deploy", str(ROOT / "infra" / "modal" / "modal_deploy.py")], check=True)
    else:
        print("Run:")
        print("  ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_deploy.py")
