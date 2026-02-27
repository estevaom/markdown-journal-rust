use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

mod template_filter;
use template_filter::TemplateFilter;

mod embeddings_http;
use embeddings_http::EmbeddingGenerator;

mod keyword_tantivy;
use keyword_tantivy::KeywordIndex;

mod usearch_store;
use usearch_store::{ChunkMetadata, USearchStore};

#[derive(Parser, Debug)]
#[command(author, version, about = "Index journal files for RAG search (USearch + SQLite)", long_about = None)]
struct Args {
    /// Journal directory to index
    #[arg(short, long, default_value = "journal")]
    journal_dir: PathBuf,

    /// Data directory for USearch + SQLite metadata
    #[arg(short = 'd', long, default_value = ".tech/data/usearch")]
    data_dir: PathBuf,

    /// Force rebuild entire index
    #[arg(short, long)]
    rebuild: bool,

    /// Only index files modified since this date (YYYY-MM-DD)
    #[arg(short, long)]
    since: Option<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Deserialize)]
struct Frontmatter {
    date: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("🔍 RAG Indexer (USearch)");
    println!("📁 Scanning: {}", args.journal_dir.display());
    println!("💾 Index location: {}", args.data_dir.display());

    fs::create_dir_all(&args.data_dir)?;

    let documents = scan_journal_directory(&args.journal_dir, args.since.as_deref(), args.verbose)?;
    println!("\n📊 Found {} documents", documents.len());

    if documents.is_empty() {
        println!("No documents to index!");
        return Ok(());
    }

    let filter = TemplateFilter::new();

    let embedding_generator = EmbeddingGenerator::new()?;
    let embedding_dim = embedding_generator.embedding_dimension();
    println!("  Embedding dimension: {}", embedding_dim);

    let index_path = args.data_dir.join("journal_vectors.usearch");
    let metadata_path = args.data_dir.join("journal_metadata.db");
    let keyword_dir = args.data_dir.join("keyword.tantivy");

    let full_rebuild = args.rebuild || !index_path.exists() || !metadata_path.exists();

    let mut store = if full_rebuild {
        if index_path.exists() {
            println!("🗑️  Removing existing index...");
            fs::remove_file(&index_path)?;
        }
        if metadata_path.exists() {
            fs::remove_file(&metadata_path)?;
        }
        println!("🆕 Creating new index...");
        USearchStore::new(&index_path, &metadata_path, embedding_dim)?
    } else {
        println!("📂 Loading existing index...");
        USearchStore::load(&index_path, &metadata_path, embedding_dim)?
    };

    let (mut keyword_index, keyword_created) = if full_rebuild {
        (KeywordIndex::create_fresh(&keyword_dir)?, true)
    } else {
        KeywordIndex::open_or_create(&keyword_dir)?
    };

    let mut next_id: u64 = if full_rebuild { 0 } else { store.get_max_chunk_id()? + 1 };

    if args.verbose && next_id > 0 {
        println!("  Starting chunk IDs from: {}", next_id);
    }

    let (docs_to_process, skipped_count) = if full_rebuild {
        (documents.clone(), 0)
    } else {
        filter_documents_for_indexing(&mut store, &documents, args.verbose)?
    };

    let deleted_paths = if full_rebuild {
        Vec::new()
    } else {
        cleanup_deleted_files(&mut store, &documents, args.verbose)?
    };
    let deleted_count = deleted_paths.len();

    // If we created a fresh keyword index on an existing vector index, bootstrap it from the
    // existing chunks in SQLite so keyword/hybrid search works immediately (no full re-embed).
    if keyword_created && !full_rebuild {
        bootstrap_keyword_index_from_metadata(&mut keyword_index, &store, args.verbose)?;
    }

    // Keep keyword index in sync with deletions/modifications before we add new chunks.
    if !full_rebuild {
        for doc in &docs_to_process {
            keyword_index.delete_path(&doc.path);
        }
        for deleted_path in &deleted_paths {
            keyword_index.delete_path(deleted_path);
        }
    }

    println!(
        "\n📊 Processing {} documents ({} unchanged, {} deleted)",
        docs_to_process.len(),
        skipped_count,
        deleted_count
    );

    if docs_to_process.is_empty() && deleted_count == 0 && !full_rebuild {
        if keyword_created {
            println!("💾 Committing keyword index...");
            keyword_index.commit()?;
            println!("✨ Keyword index bootstrapped (no vector changes).");
        } else {
            println!("✨ No new or modified documents to index!");
        }
        return Ok(());
    }

    println!("\n🧽 Cleaning template noise and chunking documents...");
    println!("🤖 Generating embeddings via embedding service...");

    let mut all_chunks = Vec::new();
    let mut chunk_metadata_list = Vec::new();

    for doc in &docs_to_process {
        let chunks = filter.extract_chunks(&doc.content, 2000);
        let num_chunks = chunks.len() as i32;

        for (idx, chunk_content) in chunks.into_iter().enumerate() {
            let metadata = ChunkMetadata {
                id: next_id,
                path: doc.path.clone(),
                date: doc.date,
                content: chunk_content.clone(),
                chunk_index: idx as i32,
                total_chunks: num_chunks,
            };

            all_chunks.push(chunk_content);
            chunk_metadata_list.push(metadata);
            next_id += 1;
        }
    }

    println!(
        "  Extracted {} chunks from {} documents",
        all_chunks.len(),
        docs_to_process.len()
    );

    let mut embeddings = Vec::new();
    let batch_size = 100;

    for (i, chunk_batch) in all_chunks.chunks(batch_size).enumerate() {
        print!(
            "  Generating embeddings batch {}/{}...\r",
            i + 1,
            (all_chunks.len() + batch_size - 1) / batch_size
        );
        std::io::stdout().flush()?;

        let batch_embeddings = embedding_generator.generate_embeddings(
            chunk_batch.iter().map(|s| s.to_string()).collect(),
        )?;
        embeddings.extend(batch_embeddings);
    }

    println!(
        "\n✅ Generated {} embeddings of dimension {}",
        embeddings.len(),
        embedding_dim
    );

    println!("📥 Adding chunks to USearch index...");
    let mut chunks_by_file: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    for (metadata, embedding) in chunk_metadata_list.into_iter().zip(embeddings.iter()) {
        let path = metadata.path.clone();
        keyword_index.add_chunk(&metadata)?;
        store.add_chunk(metadata, embedding)?;
        *chunks_by_file.entry(path).or_insert(0) += 1;
    }

    for doc in &docs_to_process {
        let chunk_count = chunks_by_file.get(&doc.path).copied().unwrap_or(0);
        let mtime = get_file_mtime(&doc.path)?;
        store.update_file_mtime(&doc.path, mtime, chunk_count)?;
    }

    println!("💾 Saving index to disk...");
    store.save(&index_path)?;

    println!("💾 Committing keyword index...");
    keyword_index.commit()?;

    let total_chunks = store.len()?;
    println!(
        "✅ Indexed {} chunks ({} documents processed)",
        total_chunks,
        docs_to_process.len()
    );
    if skipped_count > 0 {
        println!("⏭️  Skipped {} unchanged documents", skipped_count);
    }

    println!("\n✨ Indexing complete!");
    println!("  Index: {}", index_path.display());
    println!("  Metadata: {}", metadata_path.display());
    println!("  Keyword index: {}", keyword_dir.display());

    Ok(())
}

fn bootstrap_keyword_index_from_metadata(
    keyword_index: &mut KeywordIndex,
    store: &USearchStore,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("🔤 Bootstrapping keyword index from existing metadata DB...");
    }

    let total: i64 = store
        .metadata_db
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
    if verbose {
        println!("  Indexing {} existing chunks (keyword-only)...", total);
    }

    let mut stmt = store.metadata_db.prepare("SELECT id, path, content FROM chunks")?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)? as u64,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in rows {
        let (id, path, content) = row?;
        keyword_index.add_raw(id, &path, &content)?;
    }

    Ok(())
}

fn get_file_date(path: &Path, verbose: bool) -> Result<NaiveDate> {
    use chrono::{DateTime, Utc};

    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?;
    let datetime: DateTime<Utc> = modified.into();
    let date = datetime.naive_utc().date();

    if verbose {
        println!("    → File modified date: {}", date);
    }

    Ok(date)
}

fn scan_journal_directory(dir: &Path, since: Option<&str>, verbose: bool) -> Result<Vec<ScanDocument>> {
    let mut documents = Vec::new();
    let matter = Matter::<YAML>::new();

    let since_date = since
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()?;

    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        if path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with("template"))
            .unwrap_or(false)
        {
            continue;
        }

        if verbose {
            println!("  Checking: {}", path.display());
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  ⚠️  Error reading {}: {}", path.display(), e);
                continue;
            }
        };

        let parsed = matter.parse(&content);

        let date = if let Some(data) = &parsed.data {
            if let Ok(fm) = data.deserialize::<Frontmatter>() {
                match NaiveDate::parse_from_str(&fm.date, "%Y-%m-%d") {
                    Ok(date) => date,
                    Err(e) => {
                        eprintln!(
                            "  ⚠️  Invalid date in frontmatter for {}: {}, using file modification time",
                            path.display(),
                            e
                        );
                        get_file_date(path, verbose)?
                    }
                }
            } else {
                if verbose {
                    println!(
                        "  📅 Using file modification time for: {} (unparseable frontmatter)",
                        path.display()
                    );
                }
                get_file_date(path, verbose)?
            }
        } else {
            if verbose {
                println!(
                    "  📅 Using file modification time for: {} (no frontmatter)",
                    path.display()
                );
            }
            get_file_date(path, verbose)?
        };

        if let Some(since) = since_date {
            if date < since {
                if verbose {
                    println!("  ⏭️  Skipping {} (older than {})", path.display(), since);
                }
                continue;
            }
        }

        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        let days_since_epoch = (date - epoch).num_days() as i32;

        documents.push(ScanDocument {
            path: path.to_string_lossy().to_string(),
            date: days_since_epoch,
            content: parsed.content,
        });
    }

    documents.sort_by_key(|d| d.date);

    Ok(documents)
}

#[derive(Clone)]
struct ScanDocument {
    path: String,
    date: i32,
    content: String,
}

fn get_file_mtime(path: &str) -> Result<i64> {
    use std::time::SystemTime;

    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?;
    let duration = modified.duration_since(SystemTime::UNIX_EPOCH)?;
    Ok(duration.as_secs() as i64)
}

fn filter_documents_for_indexing(
    store: &mut USearchStore,
    documents: &[ScanDocument],
    verbose: bool,
) -> Result<(Vec<ScanDocument>, usize)> {
    let mut docs_to_process = Vec::new();
    let mut skipped_count = 0;

    for doc in documents {
        let current_mtime = get_file_mtime(&doc.path)?;

        match store.get_file_mtime(&doc.path)? {
            Some(stored_mtime) => {
                if current_mtime != stored_mtime {
                    if verbose {
                        println!("  Modified: {}", doc.path);
                    }
                    let removed = store.remove_file_chunks(&doc.path)?;
                    if verbose {
                        println!("    Removed {} old chunks", removed);
                    }
                    docs_to_process.push(doc.clone());
                } else {
                    if verbose {
                        println!("  Skipping unchanged: {}", doc.path);
                    }
                    skipped_count += 1;
                }
            }
            None => {
                if verbose {
                    println!("  New file: {}", doc.path);
                }
                docs_to_process.push(doc.clone());
            }
        }
    }

    Ok((docs_to_process, skipped_count))
}

fn cleanup_deleted_files(
    store: &mut USearchStore,
    current_documents: &[ScanDocument],
    verbose: bool,
) -> Result<Vec<String>> {
    let indexed_paths = store.get_all_indexed_paths()?;

    let current_paths: std::collections::HashSet<_> =
        current_documents.iter().map(|doc| doc.path.as_str()).collect();

    let mut deleted_paths = Vec::new();

    for indexed_path in indexed_paths {
        if !current_paths.contains(indexed_path.as_str()) {
            if verbose {
                println!("  Deleted: {}", indexed_path);
            }

            let removed = store.remove_file_chunks(&indexed_path)?;
            if verbose {
                println!("    Removed {} chunks", removed);
            }

            store.metadata_db.execute(
                "DELETE FROM indexed_files WHERE path = ?1",
                rusqlite::params![&indexed_path],
            )?;

            deleted_paths.push(indexed_path);
        }
    }

    Ok(deleted_paths)
}
