#!/usr/bin/env bash
set -euo pipefail

# Setup script for Ubuntu/Debian (including WSL)

print_message() {
  echo "----------------------------------------------------"
  echo "$1"
  echo "----------------------------------------------------"
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

print_message "Installing system dependencies..."
sudo apt update
sudo apt install -y build-essential python3 python3-venv python3-pip

print_message "Installing Rust via rustup (if needed)..."
if ! command -v rustc >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

print_message "Setting up embedding service (Python venv)..."
if [ -d "embedding_service" ]; then
  python3 -m venv embedding_service/.venv
  # shellcheck disable=SC1091
  source embedding_service/.venv/bin/activate
  pip install -U pip
  pip install -r embedding_service/requirements.txt
  deactivate
else
  echo "WARNING: embedding_service/ not found. Index/search will not work without embeddings."
fi

print_message "Building Rust CLI tools..."

if [ -d ".tech/code/rust_scripts/frontmatter_query" ]; then
  (cd .tech/code/rust_scripts/frontmatter_query && cargo build --release)
fi

if [ -d ".tech/code/rust_scripts/rag_search" ]; then
  (cd .tech/code/rust_scripts/rag_search && cargo build --release)
fi

print_message "Ensuring helper scripts are executable..."
chmod +x mjr search-rag.sh query-frontmatter.sh index-journal.sh reindex-rag.sh start-server.sh stop-server.sh generate-weekly-weight-graph.sh bin/yt-transcript .tech/code/scripts/weight-analysis/setup_venv.sh 2>/dev/null || true

if grep -qi microsoft /proc/version 2>/dev/null; then
  echo ""
  echo "Note: WSL environment detected."
  echo "For best performance, keep your journal files inside the WSL filesystem (not on /mnt/c)."
fi

print_message "Setup complete!"
echo ""
echo "Next steps:"
echo "1) Start embeddings: ./start-server.sh"
echo "2) Index journal:    ./index-journal.sh"
echo "3) Search:           ./search-rag.sh \"your query\""
echo "4) Frontmatter:      ./query-frontmatter.sh --fields mood anxiety --format table"
