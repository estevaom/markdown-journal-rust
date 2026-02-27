use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub id: u64,
    pub path: String,
    pub date: i32, // Days since epoch
    pub content: String,
    pub chunk_index: i32,
    pub total_chunks: i32,
}

pub struct USearchStore {
    index: Index,
    pub(crate) metadata_db: Connection,
    embedding_dim: usize,
}

impl USearchStore {
    pub fn new(_index_path: &Path, metadata_path: &Path, embedding_dim: usize) -> Result<Self> {
        let options = IndexOptions {
            dimensions: embedding_dim,
            metric: MetricKind::IP, // Inner Product for cosine similarity (when embeddings are normalized)
            quantization: ScalarKind::F32,
            ..Default::default()
        };

        let index = usearch::new_index(&options)?;

        // Reserve some initial capacity to avoid resize overhead and potential segfaults.
        index.reserve(5000)?;

        let metadata_db = Connection::open(metadata_path)?;

        metadata_db.execute(
            "CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL,
                date INTEGER NOT NULL,
                content TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                total_chunks INTEGER NOT NULL
            )",
            [],
        )?;

        metadata_db.execute("CREATE INDEX IF NOT EXISTS idx_date ON chunks(date)", [])?;

        // Track per-file mtime/chunk count to support incremental indexing.
        metadata_db.execute(
            "CREATE TABLE IF NOT EXISTS indexed_files (
                path TEXT PRIMARY KEY,
                mtime INTEGER NOT NULL,
                chunk_count INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self {
            index,
            metadata_db,
            embedding_dim,
        })
    }

    pub fn load(index_path: &Path, metadata_path: &Path, embedding_dim: usize) -> Result<Self> {
        let options = IndexOptions {
            dimensions: embedding_dim,
            metric: MetricKind::IP,
            quantization: ScalarKind::F32,
            ..Default::default()
        };

        let index = usearch::new_index(&options)?;
        index.load(index_path.to_str().unwrap())?;

        // Reserve additional capacity for new vectors
        index.reserve(1000)?;

        let metadata_db = Connection::open(metadata_path)?;

        Ok(Self {
            index,
            metadata_db,
            embedding_dim,
        })
    }

    pub fn add_chunk(&mut self, metadata: ChunkMetadata, embedding: &[f32]) -> Result<()> {
        if embedding.len() != self.embedding_dim {
            return Err(anyhow::anyhow!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.embedding_dim,
                embedding.len()
            ));
        }

        self.index.add(metadata.id, embedding)?;

        self.metadata_db.execute(
            "INSERT OR REPLACE INTO chunks (id, path, date, content, chunk_index, total_chunks)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                metadata.id as i64,
                metadata.path,
                metadata.date,
                metadata.content,
                metadata.chunk_index,
                metadata.total_chunks,
            ],
        )?;

        Ok(())
    }

    pub fn save(&self, index_path: &Path) -> Result<()> {
        self.index.save(index_path.to_str().unwrap())?;
        Ok(())
    }

    pub fn len(&self) -> Result<usize> {
        let count: i64 = self
            .metadata_db
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn get_file_mtime(&self, path: &str) -> Result<Option<i64>> {
        let result = self.metadata_db.query_row(
            "SELECT mtime FROM indexed_files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        );

        match result {
            Ok(mtime) => Ok(Some(mtime)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn update_file_mtime(&mut self, path: &str, mtime: i64, chunk_count: i32) -> Result<()> {
        self.metadata_db.execute(
            "INSERT OR REPLACE INTO indexed_files (path, mtime, chunk_count) VALUES (?1, ?2, ?3)",
            params![path, mtime, chunk_count],
        )?;
        Ok(())
    }

    pub fn remove_file_chunks(&mut self, path: &str) -> Result<usize> {
        // NOTE: USearch does not currently expose a stable delete API.
        // We delete metadata rows so results are ignored, but the vector index remains append-only.
        // For a fully compact index, run with --rebuild occasionally.
        let removed_count = self.metadata_db.execute(
            "DELETE FROM chunks WHERE path = ?1",
            params![path],
        )?;

        Ok(removed_count)
    }

    pub fn get_all_indexed_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.metadata_db.prepare("SELECT path FROM indexed_files")?;
        let paths = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(paths)
    }

    pub fn get_max_chunk_id(&self) -> Result<u64> {
        let result: i64 = self.metadata_db.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM chunks",
            [],
            |row| row.get(0),
        )?;
        Ok(result as u64)
    }
}

