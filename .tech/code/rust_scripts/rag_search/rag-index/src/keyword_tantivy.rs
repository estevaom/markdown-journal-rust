use anyhow::Result;
use std::fs;
use std::path::Path;
use tantivy::schema::{Schema, STORED, STRING, TEXT};
use tantivy::{Index, IndexWriter, TantivyDocument, Term};

use crate::usearch_store::ChunkMetadata;

pub struct KeywordIndex {
    index: Index,
    writer: IndexWriter,
    chunk_id_field: tantivy::schema::Field,
    path_field: tantivy::schema::Field,
    content_field: tantivy::schema::Field,
}

impl KeywordIndex {
    /// Open an existing keyword index, or create a fresh one if missing/invalid.
    /// Returns `(index, created_fresh)`.
    pub fn open_or_create(dir: &Path) -> Result<(Self, bool)> {
        if dir.exists() {
            match Index::open_in_dir(dir) {
                Ok(index) => match Self::from_index(index) {
                    Ok(ok) => Ok((ok, false)),
                    Err(_) => Ok((Self::create_fresh(dir)?, true)),
                },
                Err(_) => Ok((Self::create_fresh(dir)?, true)),
            }
        } else {
            Ok((Self::create_fresh(dir)?, true))
        }
    }

    pub fn create_fresh(dir: &Path) -> Result<Self> {
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        fs::create_dir_all(dir)?;

        let schema = Self::schema();
        let index = Index::create_in_dir(dir, schema)?;
        Self::from_index(index)
    }

    pub fn delete_path(&mut self, path: &str) {
        let term = Term::from_field_text(self.path_field, path);
        self.writer.delete_term(term);
    }

    pub fn add_raw(&mut self, id: u64, path: &str, content: &str) -> Result<()> {
        let mut doc = TantivyDocument::default();
        doc.add_u64(self.chunk_id_field, id);
        doc.add_text(self.path_field, path);
        doc.add_text(self.content_field, content);
        self.writer.add_document(doc)?;
        Ok(())
    }

    pub fn add_chunk(&mut self, metadata: &ChunkMetadata) -> Result<()> {
        self.add_raw(metadata.id, &metadata.path, &metadata.content)?;
        Ok(())
    }

    pub fn commit(mut self) -> Result<()> {
        self.writer.commit()?;
        Ok(())
    }

    fn from_index(index: Index) -> Result<Self> {
        let schema = index.schema();

        let chunk_id_field = schema.get_field("chunk_id")?;
        let path_field = schema.get_field("path")?;
        let content_field = schema.get_field("content")?;

        let writer = index.writer(50_000_000)?;

        Ok(Self {
            index,
            writer,
            chunk_id_field,
            path_field,
            content_field,
        })
    }

    fn schema() -> Schema {
        let mut schema_builder = Schema::builder();
        schema_builder.add_u64_field("chunk_id", STORED);
        schema_builder.add_text_field("path", STRING | STORED);
        schema_builder.add_text_field("content", TEXT);
        schema_builder.build()
    }
}

