# Markdown Journal RAG

A toolkit for searching and analyzing your Markdown journal entries locally. Combines semantic search, keyword search, and frontmatter analytics -- all running on your machine.

## Features

- **Semantic search** over Markdown using a local vector index (USearch + SQLite metadata)
- **Keyword search** (BM25 via Tantivy)
- **Hybrid search** (semantic + keyword fused with RRF)
- **Incremental indexing** (only new/changed files) with `--rebuild` for full refresh
- **Frontmatter analytics** (query fields, stats, tag/trigger linting, streak helpers)
- **Local by default**: journal files, embeddings, and indexes stay on your machine
- **Cross-platform**: macOS, Ubuntu/Debian (including WSL), Arch Linux
- **Zero config builds**: wrapper scripts auto-build missing Rust binaries (including macOS C++ header fixups)

## Quick Start

### 1. Install + build (pick one)

**macOS:**
```bash
./setup_mac_environment.sh
```

**Ubuntu/Debian (including WSL):**
```bash
./setup_ubuntu_environment.sh
```

**Arch Linux:**
```bash
./setup_arch_environment.sh
```

### 2. Start the embedding service

```bash
./start-server.sh
```

This downloads the embedding model on first run and then serves:
- `POST /embed`
- `GET /health`
- `GET /info`

### 3. Index and search

```bash
# Incremental index (fast for daily updates)
./index-journal.sh

# Full rebuild (use occasionally to compact the index)
./reindex-rag.sh

# Search
./search-rag.sh "project retrospective" -n 10
./search-rag.sh "learning rust ownership" --after 2025-06-01
./search-rag.sh "meeting notes" --files-only

# Search modes (default: hybrid)
./search-rag.sh "postgres deadlocks" --mode keyword
./search-rag.sh "what did I learn this week?" --mode semantic

# Frontmatter query
./query-frontmatter.sh --fields mood anxiety weight_kg --format table
./query-frontmatter.sh --list-tags --format json
./query-frontmatter.sh --lint --last-days 30 --format json
```

If release binaries are missing, `index-journal.sh`, `search-rag.sh`, and `query-frontmatter.sh` build them automatically before running.

## Utilities

### Health-check + quick capture

```bash
./mjr doctor
./mjr doctor --quick
./mjr inbox "Idea: write about that meeting"
```

### Templates

- `template/daily.md`: weekday daily template (used when creating new daily files)
- `template/weekend.md`: weekend template (used on Sat/Sun when creating new daily files)
- `template/daily_summary.md`: rolling weekly cache format for daily summaries
- `template/person.md`: a starting point for "people notes"

### YouTube transcript -> text

```bash
./bin/yt-transcript "https://www.youtube.com/watch?v=..." > transcript.txt
```

### Frontmatter graph generator

Generates a progress graph from any numeric frontmatter field. Ships with a `weight_kg` example -- adapt the script to plot mood, sleep, anxiety, or anything else you track.

```bash
# Generate weight graph (writes weight_progress.png at repo root)
./generate-weekly-weight-graph.sh

# Or run the Python script directly for customization:
cd .tech/code/scripts/weight-analysis
./setup_venv.sh
source .venv/bin/activate
python weight_analysis.py --output ../../../../weight_progress.png
deactivate
```

## Manual Setup

### Rust tools

```bash
# Uses a cross-platform helper. On macOS, this also applies SDK-backed libc++ includes.
source bin/rust_build_helpers.sh
cargo_build_release .tech/code/rust_scripts/rag_search
cargo_build_release .tech/code/rust_scripts/frontmatter_query
```

### Embedding service

```bash
cd embedding_service
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install -r requirements.txt
```

Run it from repo root:
```bash
./start-server.sh
```

## Configuration

### Embedding service

- `EMBEDDING_SERVICE_URL`: Rust tools connect here (default: `http://127.0.0.1:8000`)
- `EMBEDDING_SERVICE_HOST`: embedding service bind host for `start-server.sh` (default: `127.0.0.1`)
- `EMBEDDING_SERVICE_PORT`: embedding service bind port for `start-server.sh` (default: `8000`)
- `EMBEDDING_MODEL`: sentence-transformers model name (default: `BAAI/bge-large-en-v1.5`)
- `EMBEDDING_DEVICE`: `cuda`, `mps`, or `cpu` (default: auto-detect)

### Notes on CUDA

The setup scripts install a CPU PyTorch wheel by default. For CUDA acceleration, install a CUDA-enabled PyTorch wheel (see PyTorch install docs) and then run the service with `EMBEDDING_DEVICE=cuda`.

## Directory Structure

```
markdown-journal-rust/
├── README.md
├── CLAUDE.md
├── mjr
├── bin/
│   ├── rust_build_helpers.sh
│   └── yt-transcript
├── setup_mac_environment.sh
├── setup_ubuntu_environment.sh
├── setup_arch_environment.sh
├── start-server.sh
├── stop-server.sh
├── index-journal.sh
├── reindex-rag.sh
├── search-rag.sh
├── query-frontmatter.sh
├── generate-weekly-weight-graph.sh
├── embedding_service/                 # HTTP embedding server (FastAPI)
├── template/                         # Journal templates
├── journal/                           # Sample journal entries (safe to replace with your own)
└── .tech/
    ├── code/
    │   ├── scripts/
    │   │   └── weight-analysis/      # Weight progress graph tooling (Python)
    │   └── rust_scripts/
    │       ├── rag_search/            # rag-index + rag-search
    │       └── frontmatter_query/     # frontmatter-query
    └── data/
        └── usearch/                   # Generated index + metadata (not committed)
```

## Privacy

- Journal files and indexes are stored locally.
- Nothing is sent to third-party APIs by default.
- The embedding service runs locally on `127.0.0.1` unless you change it.

## Claude Code Integration

This repo includes optional [Claude Code](https://docs.anthropic.com/en/docs/claude-code) integration in `.claude/`:

- **Slash commands:** `/start` (morning workflow), `/commit` (end-of-day completion + push), `/complete_previous_day` (recover a missed day)
- **Agents:** daily summary, field completer, RAG search, weekly retro analyzer

These are optional -- the search tools, indexer, and scripts work without Claude Code. See `CLAUDE.md` for details.
