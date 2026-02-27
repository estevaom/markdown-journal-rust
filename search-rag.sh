#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"
# shellcheck disable=SC1091
source "$ROOT_DIR/bin/rust_build_helpers.sh"

BIN=".tech/code/rust_scripts/rag_search/target/release/rag-search"
RAG_DIR=".tech/code/rust_scripts/rag_search"

if [ ! -f "$BIN" ]; then
  echo "❌ rag-search binary not found. Building..." >&2
  cargo_build_release "$RAG_DIR" --bin rag-search
fi

exec "$BIN" "$@"
