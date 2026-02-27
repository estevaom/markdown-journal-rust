use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;

mod embeddings_http;
use embeddings_http::EmbeddingGenerator;

mod keyword_tantivy;

#[derive(Parser, Debug)]
#[command(author, version, about = "Search indexed journal files (semantic + keyword)", long_about = None)]
struct Args {
    /// Search query
    query: String,

    /// Filter results after this date (YYYY-MM-DD)
    #[arg(long)]
    after: Option<String>,

    /// Filter results before this date (YYYY-MM-DD)
    #[arg(long)]
    before: Option<String>,

    /// Number of results to return
    #[arg(short, long, default_value = "10")]
    num_results: usize,

    /// Return only unique file paths
    #[arg(long)]
    files_only: bool,

    /// Show debug information (scores, metadata)
    #[arg(long)]
    debug: bool,

    /// Search mode: semantic (vector), keyword (BM25), or hybrid (RRF fusion)
    #[arg(long, value_enum, default_value = "hybrid")]
    mode: SearchMode,

    /// Output format
    #[arg(short, long, default_value = "text", value_enum)]
    format: OutputFormat,

    /// USearch data directory
    #[arg(short = 'd', long, default_value = ".tech/data/usearch")]
    data_dir: PathBuf,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Serialize, Clone)]
struct SearchResult {
    id: u64,
    path: PathBuf,
    date: NaiveDate,
    score: f32,
    snippet: String,
    chunk_index: i32,
    total_chunks: i32,
}

#[derive(Debug)]
struct ChunkMetadata {
    id: u64,
    path: String,
    date: i32,
    content: String,
    chunk_index: i32,
    total_chunks: i32,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum SearchMode {
    Semantic,
    Keyword,
    Hybrid,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let after_date = args
        .after
        .as_deref()
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()?;

    let before_date = args
        .before
        .as_deref()
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()?;

    if args.debug {
        eprintln!("🔍 Query: '{}'", args.query);
        if let Some(after) = after_date {
            eprintln!("📅 After: {}", after);
        }
        if let Some(before) = before_date {
            eprintln!("📅 Before: {}", before);
        }
        eprintln!("🧭 Mode: {:?}", args.mode);
    }

    let results = match args.mode {
        SearchMode::Semantic => {
            search_usearch(
                &args.data_dir,
                &args.query,
                after_date,
                before_date,
                args.num_results,
                args.debug,
            )
            .await?
        }
        SearchMode::Keyword => {
            search_keyword(
                &args.data_dir,
                &args.query,
                after_date,
                before_date,
                args.num_results,
                args.debug,
            )
            .await?
        }
        SearchMode::Hybrid => {
            search_hybrid(
                &args.data_dir,
                &args.query,
                after_date,
                before_date,
                args.num_results,
                args.debug,
            )
            .await?
        }
    };

    match args.format {
        OutputFormat::Text => {
            if args.files_only {
                let mut seen_paths = std::collections::HashSet::new();
                for result in &results {
                    if seen_paths.insert(result.path.clone()) {
                        println!("{}", result.path.display());
                    }
                }
            } else {
                let score_label = match args.mode {
                    SearchMode::Semantic => "Score",
                    SearchMode::Keyword => "BM25",
                    SearchMode::Hybrid => "RRF",
                };
                for (i, result) in results.iter().enumerate() {
                    println!(
                        "\n{} {} | {} | {}: {:.3} | Chunk {}/{}",
                        i + 1,
                        result.date,
                        result.path.display(),
                        score_label,
                        result.score,
                        result.chunk_index + 1,
                        result.total_chunks
                    );
                    println!("  {}", result.snippet);
                }
            }
        }
        OutputFormat::Json => {
            let output = serde_json::to_string_pretty(&results)?;
            println!("{}", output);
        }
    }

    Ok(())
}

async fn search_usearch(
    data_dir: &PathBuf,
    query: &str,
    after: Option<NaiveDate>,
    before: Option<NaiveDate>,
    limit: usize,
    debug: bool,
) -> Result<Vec<SearchResult>> {
    let index_path = data_dir.join("journal_vectors.usearch");
    let metadata_path = data_dir.join("journal_metadata.db");

    if !index_path.exists() || !metadata_path.exists() {
        return Err(anyhow::anyhow!(
            "Index not found at {}. Run ./index-journal.sh first.",
            data_dir.display()
        ));
    }

    if debug {
        eprintln!("🤖 Requesting query embedding...");
    }
    let embedding_generator = EmbeddingGenerator::new()?;
    let query_embedding = embedding_generator.generate_embedding(query)?;

    if debug {
        eprintln!("✅ Generated query embedding (dim: {})", query_embedding.len());
        eprintln!("📂 Loading USearch index...");
    }

    let options = usearch::IndexOptions {
        dimensions: query_embedding.len(),
        metric: usearch::MetricKind::IP,
        quantization: usearch::ScalarKind::F32,
        ..Default::default()
    };

    let index = usearch::new_index(&options)?;
    index.load(index_path.to_str().unwrap())?;

    if debug {
        eprintln!("✅ Index loaded");
    }

    let search_limit = limit * 3;
    let search_results = index.search(&query_embedding, search_limit)?;

    if debug {
        eprintln!("🔍 Found {} initial matches", search_results.keys.len());
    }

    let metadata_db = Connection::open(&metadata_path)?;

    let after_days = after.map(|d| {
        (d - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days() as i32
    });
    let before_days = before.map(|d| {
        (d - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days() as i32
    });

    let mut results = Vec::new();

    for (key, distance) in search_results
        .keys
        .iter()
        .zip(search_results.distances.iter())
    {
        let metadata = get_metadata(&metadata_db, *key)?;

        if let Some(meta) = metadata {
            if let Some(after) = after_days {
                if meta.date < after {
                    continue;
                }
            }
            if let Some(before) = before_days {
                if meta.date > before {
                    continue;
                }
            }

            let date = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap() + chrono::Duration::days(meta.date as i64);
            let snippet = extract_snippet(&meta.content, query, 500);

            results.push(SearchResult {
                id: *key,
                path: PathBuf::from(meta.path),
                date,
                score: *distance, // inner product similarity (higher is better)
                snippet,
                chunk_index: meta.chunk_index,
                total_chunks: meta.total_chunks,
            });

            if results.len() >= limit {
                break;
            }
        }
    }

    Ok(results)
}

async fn search_keyword(
    data_dir: &PathBuf,
    query: &str,
    after: Option<NaiveDate>,
    before: Option<NaiveDate>,
    limit: usize,
    debug: bool,
) -> Result<Vec<SearchResult>> {
    let keyword_dir = data_dir.join("keyword.tantivy");
    let metadata_path = data_dir.join("journal_metadata.db");

    if !keyword_dir.exists() {
        return Err(anyhow::anyhow!(
            "Keyword index not found at {}. Run ./index-journal.sh first.",
            keyword_dir.display()
        ));
    }
    if !metadata_path.exists() {
        return Err(anyhow::anyhow!(
            "Metadata DB not found at {}. Run ./index-journal.sh first.",
            metadata_path.display()
        ));
    }

    if debug {
        eprintln!("🔤 Running keyword search (BM25)...");
    }

    let search_limit = limit * 3;
    let candidates = keyword_tantivy::search(&keyword_dir, query, search_limit)?;

    if debug {
        eprintln!("🔎 Keyword candidates: {}", candidates.len());
    }

    let metadata_db = Connection::open(&metadata_path)?;

    let after_days = after.map(|d| {
        (d - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days() as i32
    });
    let before_days = before.map(|d| {
        (d - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days() as i32
    });

    let mut results = Vec::new();

    for (id, score) in candidates {
        let metadata = get_metadata(&metadata_db, id)?;
        if let Some(meta) = metadata {
            if let Some(after) = after_days {
                if meta.date < after {
                    continue;
                }
            }
            if let Some(before) = before_days {
                if meta.date > before {
                    continue;
                }
            }

            let date = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
                + chrono::Duration::days(meta.date as i64);
            let snippet = extract_snippet(&meta.content, query, 500);

            results.push(SearchResult {
                id,
                path: PathBuf::from(meta.path),
                date,
                score,
                snippet,
                chunk_index: meta.chunk_index,
                total_chunks: meta.total_chunks,
            });

            if results.len() >= limit {
                break;
            }
        }
    }

    Ok(results)
}

async fn search_hybrid(
    data_dir: &PathBuf,
    query: &str,
    after: Option<NaiveDate>,
    before: Option<NaiveDate>,
    limit: usize,
    debug: bool,
) -> Result<Vec<SearchResult>> {
    let engine_limit = limit * 3;

    let semantic_results = search_usearch(data_dir, query, after, before, engine_limit, debug).await?;

    let keyword_results = match search_keyword(data_dir, query, after, before, engine_limit, debug).await {
        Ok(r) => r,
        Err(e) => {
            if debug {
                eprintln!("⚠️  Keyword search unavailable: {}", e);
            }
            Vec::new()
        }
    };

    if semantic_results.is_empty() && keyword_results.is_empty() {
        return Ok(Vec::new());
    }

    Ok(fuse_rrf(&semantic_results, &keyword_results, limit))
}

fn fuse_rrf(semantic: &[SearchResult], keyword: &[SearchResult], limit: usize) -> Vec<SearchResult> {
    use std::cmp::Ordering;
    use std::collections::HashMap;

    const K: f32 = 60.0;

    let mut fused_scores: HashMap<u64, f32> = HashMap::new();

    for (rank, result) in semantic.iter().enumerate() {
        let r = (rank as f32) + 1.0;
        *fused_scores.entry(result.id).or_insert(0.0) += 1.0 / (K + r);
    }
    for (rank, result) in keyword.iter().enumerate() {
        let r = (rank as f32) + 1.0;
        *fused_scores.entry(result.id).or_insert(0.0) += 1.0 / (K + r);
    }

    let semantic_map: HashMap<u64, &SearchResult> = semantic.iter().map(|r| (r.id, r)).collect();
    let keyword_map: HashMap<u64, &SearchResult> = keyword.iter().map(|r| (r.id, r)).collect();

    let mut ids: Vec<(u64, f32)> = fused_scores.into_iter().collect();
    ids.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    let mut out = Vec::new();
    for (id, score) in ids {
        if out.len() >= limit {
            break;
        }
        let base = keyword_map.get(&id).or_else(|| semantic_map.get(&id));
        if let Some(base) = base {
            let mut merged = (*base).clone();
            merged.score = score;
            out.push(merged);
        }
    }

    out
}

fn get_metadata(db: &Connection, id: u64) -> Result<Option<ChunkMetadata>> {
    let mut stmt = db.prepare(
        "SELECT id, path, date, content, chunk_index, total_chunks
         FROM chunks WHERE id = ?1",
    )?;

    let result = stmt.query_row(params![id as i64], |row| {
        Ok(ChunkMetadata {
            id: row.get::<_, i64>(0)? as u64,
            path: row.get(1)?,
            date: row.get(2)?,
            content: row.get(3)?,
            chunk_index: row.get(4)?,
            total_chunks: row.get(5)?,
        })
    });

    match result {
        Ok(metadata) => Ok(Some(metadata)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn extract_snippet(content: &str, query: &str, context_chars: usize) -> String {
    let lower_content = content.to_lowercase();
    let lower_query = query.to_lowercase();

    if let Some(pos) = lower_content.find(&lower_query) {
        let byte_pos = pos;
        let start_byte = byte_pos.saturating_sub(context_chars);
        let end_byte = (byte_pos + query.len() + context_chars).min(content.len());

        let start = if start_byte == 0 {
            0
        } else {
            let mut valid_start = start_byte;
            while valid_start > 0 && !content.is_char_boundary(valid_start) {
                valid_start -= 1;
            }
            valid_start
        };

        let end = if end_byte >= content.len() {
            content.len()
        } else {
            let mut valid_end = end_byte;
            while valid_end < content.len() && !content.is_char_boundary(valid_end) {
                valid_end += 1;
            }
            valid_end
        };

        let snippet = content[start..end].trim();
        if start > 0 {
            format!("...{}", snippet)
        } else {
            snippet.to_string()
        }
    } else {
        content.chars().take(context_chars * 2).collect()
    }
}
