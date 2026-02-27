# Embedding Service

Local HTTP embedding service used by the Rust indexer/searcher.

## What It Does

- Exposes `POST /embed` for embedding text batches.
- Exposes `GET /health` and `GET /info` for status/model details.

The Rust tools call this service via `EMBEDDING_SERVICE_URL` (default: `http://127.0.0.1:8000`).

This same virtualenv also installs `yt-dlp` (used by `bin/yt-transcript`).

## Configuration

- `EMBEDDING_MODEL`: Sentence-Transformers model name (default: `BAAI/bge-large-en-v1.5`)
- `EMBEDDING_DEVICE`: `cuda`, `mps`, or `cpu` (default: auto-detect)

## Install (manual)

```bash
cd embedding_service
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install -r requirements.txt
```

## Run

From the repo root:

```bash
./start-server.sh
```
