# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This repository is a local-first Journal RAG system:
- A Rust indexer (`rag-index`) that chunks Markdown files and stores vectors in a USearch index plus metadata in SQLite
- A Rust search CLI (`rag-search`) for semantic/keyword/hybrid search (date filters, JSON output, and `--files-only`)
- A Rust frontmatter analytics CLI (`frontmatter-query`)
- A local HTTP embedding service (`embedding_service/`) used by both index and search

## Architecture

### 1) Embedding Service (Python)

- Location: `embedding_service/`
- Exposes: `POST /embed`, `GET /health`, `GET /info`
- Config:
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

## Development Commands

### Build

```bash
# RAG tools
cd .tech/code/rust_scripts/rag_search
cargo build --release

# Frontmatter tool
cd ../frontmatter_query
cargo build --release
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

## Claude Slash Commands

If you use Claude Code, this repo includes:

- `/start`: bootstrap daily workflow (server + index; optional Monday retro flow)
- `/commit`: end-of-day completion + commit/push
- `/complete_previous_day`: recover a missed day, then complete + commit

## Troubleshooting

- If Rust tools can’t connect: ensure `./start-server.sh` is running and `EMBEDDING_SERVICE_URL` matches.
- First run is slower: the embedding model downloads on first service startup.
