#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

BIN=".tech/code/rust_scripts/rag_search/target/release/rag-index"
RAG_DIR=".tech/code/rust_scripts/rag_search"

if [ ! -f "$BIN" ]; then
  echo "❌ rag-index binary not found. Building..." >&2
  (cd "$RAG_DIR" && cargo build --release --bin rag-index)
fi

exec "$BIN" "$@"

