use anyhow::Result;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::{Index, TantivyDocument};

pub fn search(keyword_dir: &Path, query: &str, limit: usize) -> Result<Vec<(u64, f32)>> {
    let index = Index::open_in_dir(keyword_dir)?;
    let schema = index.schema();

    let content_field = schema.get_field("content")?;
    let chunk_id_field = schema.get_field("chunk_id")?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![content_field]);
    let query_obj = query_parser
        .parse_query(query)
        .or_else(|_| query_parser.parse_query(&format!("\"{}\"", query.replace('"', "\\\""))))?;

    let top_docs = searcher.search(&query_obj, &TopDocs::with_limit(limit))?;

    let mut out = Vec::with_capacity(top_docs.len());
    for (score, addr) in top_docs {
        let doc: TantivyDocument = searcher.doc(addr)?;
        if let Some(value) = doc.get_first(chunk_id_field).and_then(|v| v.as_u64()) {
            out.push((value, score));
        }
    }

    Ok(out)
}
