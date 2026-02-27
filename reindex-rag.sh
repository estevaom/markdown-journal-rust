#!/usr/bin/env bash
set -euo pipefail

echo "🔄 Rebuilding journal index..."
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/index-journal.sh" --rebuild "$@"
