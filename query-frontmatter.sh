#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

BIN=".tech/code/rust_scripts/frontmatter_query/target/release/frontmatter-query"
FRONTMATTER_DIR=".tech/code/rust_scripts/frontmatter_query"

if [ ! -f "$BIN" ]; then
  echo "❌ frontmatter-query binary not found. Building..." >&2
  (cd "$FRONTMATTER_DIR" && cargo build --release --bin frontmatter-query)
fi

exec "$BIN" "$@"
