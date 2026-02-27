#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_DIR="$ROOT_DIR/embedding_service"

if [ ! -d "$SERVICE_DIR" ]; then
  echo "❌ embedding_service directory not found at: $SERVICE_DIR" >&2
  exit 1
fi

if [ -x "$SERVICE_DIR/.venv/bin/python" ]; then
  # shellcheck disable=SC1091
  source "$SERVICE_DIR/.venv/bin/activate"
elif [ -x "$SERVICE_DIR/venv/bin/python" ]; then
  # shellcheck disable=SC1091
  source "$SERVICE_DIR/venv/bin/activate"
else
  echo "❌ No virtualenv found for embedding service." >&2
  echo "Run one of the setup scripts (recommended) or set it up manually:" >&2
  echo "  cd embedding_service && python3 -m venv .venv && source .venv/bin/activate && pip install -r requirements.txt" >&2
  exit 1
fi

HOST="${EMBEDDING_SERVICE_HOST:-127.0.0.1}"
PORT="${EMBEDDING_SERVICE_PORT:-8000}"

echo "🚀 Starting embedding service on http://${HOST}:${PORT}"
echo "Press Ctrl+C to stop"

cd "$SERVICE_DIR"
exec uvicorn service:app --host "$HOST" --port "$PORT"

