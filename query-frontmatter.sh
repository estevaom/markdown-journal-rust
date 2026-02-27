#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"
# shellcheck disable=SC1091
source "$ROOT_DIR/bin/rust_build_helpers.sh"

BIN=".tech/code/rust_scripts/frontmatter_query/target/release/frontmatter-query"
FRONTMATTER_DIR=".tech/code/rust_scripts/frontmatter_query"

if [ ! -f "$BIN" ]; then
  echo "❌ frontmatter-query binary not found. Building..." >&2
  cargo_build_release "$FRONTMATTER_DIR" --bin frontmatter-query
fi

exec "$BIN" "$@"
