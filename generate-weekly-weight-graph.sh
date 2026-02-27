#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEIGHT_TOOL_DIR="$ROOT_DIR/.tech/code/scripts/weight-analysis"
WEIGHT_TOOL_PY="$WEIGHT_TOOL_DIR/weight_analysis.py"

if [ ! -f "$WEIGHT_TOOL_PY" ]; then
  echo "❌ Weight analysis script not found at: $WEIGHT_TOOL_PY" >&2
  exit 1
fi

LATEST_OUT="$ROOT_DIR/weight_progress.png"

# Prefer the weight-analysis venv if available.
PYTHON="python3"
if [ -x "$WEIGHT_TOOL_DIR/.venv/bin/python" ]; then
  PYTHON="$WEIGHT_TOOL_DIR/.venv/bin/python"
elif [ -x "$WEIGHT_TOOL_DIR/venv/bin/python" ]; then
  PYTHON="$WEIGHT_TOOL_DIR/venv/bin/python"
fi

# ISO week snapshot (portable: works on macOS + Linux)
ISO_YEAR="$(date +%G)"
ISO_WEEK="$(date +%V)"

SNAP_DIR="$ROOT_DIR/journal/artifacts/weight"
SNAP_OUT="$SNAP_DIR/weight_progress_${ISO_YEAR}-W${ISO_WEEK}.png"

mkdir -p "$SNAP_DIR"

echo "📈 Generating latest weight graph..."
"$PYTHON" "$WEIGHT_TOOL_PY" --output "$LATEST_OUT"

if [ -f "$SNAP_OUT" ]; then
  echo "✅ Weekly snapshot already exists: $SNAP_OUT"
else
  cp "$LATEST_OUT" "$SNAP_OUT"
  echo "✅ Created weekly snapshot: $SNAP_OUT"
fi
