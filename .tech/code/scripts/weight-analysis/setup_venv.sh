#!/usr/bin/env bash
set -euo pipefail

echo "Setting up virtual environment for weight analysis..."

VENV_DIR=".venv"

# Backwards-compat: if a legacy venv/ exists, keep using it.
if [ -d "venv" ] && [ ! -d "$VENV_DIR" ]; then
  VENV_DIR="venv"
fi

if [ ! -d "$VENV_DIR" ]; then
  python3 -m venv "$VENV_DIR"
fi

# shellcheck disable=SC1090
source "$VENV_DIR/bin/activate"

pip install -U pip
pip install -r requirements.txt

echo ""
echo "Virtual environment setup complete."
echo "Run:"
echo "  source $VENV_DIR/bin/activate"
echo "  python weight_analysis.py --output ../../../../weight_progress.png"
echo "  deactivate"
