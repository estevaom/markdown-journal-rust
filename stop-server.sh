#!/usr/bin/env bash
set -euo pipefail

PORT="${EMBEDDING_SERVICE_PORT:-8000}"

echo "🛑 Stopping embedding service on port ${PORT}..."

if pkill -f "uvicorn service:app.*--port ${PORT}" >/dev/null 2>&1; then
  echo "✅ Embedding service stopped"
  exit 0
fi

# Fallback: older uvicorn invocation formats
if pkill -f "uvicorn service:app.*${PORT}" >/dev/null 2>&1; then
  echo "✅ Embedding service stopped"
  exit 0
fi

echo "⚠️  No embedding service found running"

