#!/usr/bin/env python3
"""
Modal Sandbox deployment for model2vec-rs-api-server.

Uses Modal Sandboxes to run the Rust binary directly without Python wrapper.
This is simpler for stateless services and avoids heartbeat/event loop issues.

Usage:
  ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_sandbox_deploy.py
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Dict

import modal

ROOT = Path(__file__).resolve().parents[2]
APP_NAME = os.getenv("MODAL_APP_NAME", "model2vec-api-sandbox")
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
    "MODAL_MEMORY_MB": "2048",
    "MODAL_TIMEOUT_SECS": "600",
    "MODAL_MIN_CONTAINERS": "0",
    "MODAL_MAX_CONTAINERS": "5",
    "MODAL_SCALEDOWN_WINDOW": "300",
    "MODAL_VOLUME_NAME": "model2vec-hf-cache",
    "MODAL_HF_CACHE_DIR": "/data/hf",
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
    return modal.Image.from_dockerfile(ROOT / "infra" / "modal" / "Dockerfile")


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
)
@modal.web_server(port=int(cfg("PORT")))
def serve_sandbox():
    """
    Run Rust HTTP server in a Modal Sandbox.

    This approach:
    1. No Python event loop complexity
    2. Rust binary runs natively
    3. Modal handles HTTP routing via port forwarding
    4. Simpler lifecycle management
    5. Extended timeout support (up to 24 hours)

    The function never returns - it blocks on the Rust server process.
    If the server crashes, Modal automatically respawns the container.
    """
    import subprocess
    import time

    print("=" * 80)
    print("🚀 Modal Sandbox: Running Rust API directly")
    print("=" * 80)

    env = os.environ.copy()
    env.update(build_env())

    print(f"📝 Environment: {len(env)} variables")
    print(f"   MODEL: {env.get('MODEL_NAME')}")
    print(f"   PORT: {env.get('PORT')}")
    print(f"   LAZY_LOAD: {env.get('LAZY_LOAD_MODEL')}")

    print(f"\n🔨 Starting: /app/model2vec-api")

    # Run the Rust binary directly
    proc = subprocess.Popen(
        ["/app/model2vec-api"],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,  # Line-buffered
    )

    print(f"✅ PID {proc.pid}\n")

    try:
        # Stream output from Rust server
        if proc.stdout:
            for line in iter(proc.stdout.readline, ""):
                if line:
                    print(f"[rust] {line.rstrip()}")

        # Wait for process
        returncode = proc.wait()
        print(f"\n❌ Server exited with code: {returncode}")

        if returncode != 0:
            raise RuntimeError(f"Server crashed with code {returncode}")

    except KeyboardInterrupt:
        print("\n⚠️  Interrupted")
        proc.terminate()
        proc.wait(timeout=5)
    except Exception as e:
        print(f"💥 Error: {e}")
        if proc.poll() is None:
            proc.kill()
        raise


@app.local_entrypoint()
def main() -> None:
    print("Modal Sandbox deployment ready.")
    print("Deploy with:")
    print("  ENV_FILE=infra/modal/.env.modal modal deploy infra/modal/modal_sandbox_deploy.py")
