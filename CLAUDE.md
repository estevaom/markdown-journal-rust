# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This repository is a local-first Journal RAG system:
- A Rust indexer (`rag-index`) that chunks Markdown files and stores vectors in a USearch index plus metadata in SQLite
- A Rust search CLI (`rag-search`) for semantic/keyword/hybrid search (date filters, JSON output, and `--files-only`)
- A Rust frontmatter analytics CLI (`frontmatter-query`)
- A local HTTP embedding service (`embedding_service/`) used by both index and search
- Shell wrapper scripts that auto-build missing Rust binaries before execution

## Architecture

### 1) Embedding Service (Python)

- Location: `embedding_service/`
- Exposes: `POST /embed`, `GET /health`, `GET /info`
- Config:
  - `EMBEDDING_SERVICE_HOST` (default: `127.0.0.1`)
  - `EMBEDDING_SERVICE_PORT` (default: `8000`)
  - `EMBEDDING_MODEL` (default: `BAAI/bge-large-en-v1.5`)
  - `EMBEDDING_DEVICE` (`cuda|mps|cpu`, default auto)

### 2) RAG Search Tools (Rust)

- Workspace: `.tech/code/rust_scripts/rag_search/`
- Binaries:
  - `rag-index`: incremental by default, `--rebuild` for full rebuild
  - `rag-search`: `--mode semantic|keyword|hybrid` (hybrid fuses USearch + BM25 via RRF)
- Data:
  - `.tech/data/usearch/journal_vectors.usearch`
  - `.tech/data/usearch/journal_metadata.db`
  - `.tech/data/usearch/keyword.tantivy/`

Important: USearch does not currently have a stable vector delete API. When files are modified/deleted, metadata rows are removed and results are ignored, but the vector index is append-only. Use `--rebuild` occasionally to compact.

### 3) Frontmatter Query Tool (Rust)

- Location: `.tech/code/rust_scripts/frontmatter_query/`
- Supports:
  - Field extraction, stats, JSON/CSV/table output
  - `--last-days`, tag/trigger filters, tag/trigger listing, linting
  - Helpers like `--last-rest-day` and `--streak`

### 4) Build Compatibility Helper (Shell)

- File: `bin/rust_build_helpers.sh`
- Function: `cargo_build_release <dir> [extra cargo args]`
- On macOS, applies SDK-based libc++ include paths for crates that compile C++ (`cxx`, `cc`).
- Used by setup scripts and by runtime wrappers (`search-rag.sh`, `index-journal.sh`, `query-frontmatter.sh`).

## Development Commands

### Build

```bash
# Cross-platform release builds (includes macOS C++ header fixups when needed)
source bin/rust_build_helpers.sh
cargo_build_release .tech/code/rust_scripts/rag_search
cargo_build_release .tech/code/rust_scripts/frontmatter_query
```

### Run (repo root)

```bash
# Start local embedding service
./start-server.sh

# Quick health-check / capture
./mjr doctor
./mjr inbox "idea to capture"

# Incremental index
./index-journal.sh

# Full rebuild
./reindex-rag.sh

# Search
./search-rag.sh "your query" -n 10
./search-rag.sh "your query" --after 2025-01-01 --before 2025-12-31 --format json
./search-rag.sh "exact identifier" --mode keyword

# Frontmatter
./query-frontmatter.sh --fields mood anxiety weight_kg --format table
./query-frontmatter.sh --lint --last-days 30 --format json
```

Note: `index-journal.sh`, `search-rag.sh`, and `query-frontmatter.sh` auto-build their binary if it is missing.

## Claude Slash Commands

If you use Claude Code, this repo includes:

- `/start`: bootstrap daily workflow (server + index; optional Monday retro flow)
- `/commit`: end-of-day completion + commit/push
- `/complete_previous_day`: recover a missed day, then complete + commit

## Troubleshooting

- If Rust tools cannot connect: ensure `./start-server.sh` is running and `EMBEDDING_SERVICE_URL` matches.
- First run is slower: the embedding model downloads on first service startup.
- If C++ header errors appear on macOS (`fatal error: 'algorithm' file not found`), build through `cargo_build_release` (or the provided wrapper scripts) instead of plain `cargo build`.
