use std::path::Path;

use anyhow::Context;
use libsql::{Builder, Connection, params};

pub async fn init_db(path: &Path) -> anyhow::Result<Connection> {
    let db = Builder::new_local(path).build().await?;
    let conn = db.connect()?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            source_path TEXT NOT NULL,
            mtime INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            document_id INTEGER NOT NULL REFERENCES documents(id),
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            embedding F32_BLOB(768)
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_embedding
            ON chunks(libsql_vector_idx(embedding, 'metric=cosine'));",
    )
    .await?;

    Ok(conn)
}

pub async fn find_document_by_path(
    conn: &Connection,
    source_path: &str,
) -> anyhow::Result<Option<(i64, i64)>> {
    let mut rows = conn
        .query(
            "SELECT id, mtime FROM documents WHERE source_path = ?1",
            params![source_path],
        )
        .await?;

    match rows.next().await? {
        Some(row) => {
            let id = row.get::<i64>(0)?;
            let mtime = row.get::<i64>(1)?;
            Ok(Some((id, mtime)))
        }
        None => Ok(None),
    }
}

pub async fn delete_document(conn: &Connection, doc_id: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM chunks WHERE document_id = ?1", params![doc_id])
        .await?;
    conn.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
        .await?;
    Ok(())
}

pub async fn insert_document(
    conn: &Connection,
    title: &str,
    source_path: &str,
    mtime: i64,
) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO documents (title, source_path, mtime) VALUES (?1, ?2, ?3)",
        params![title, source_path, mtime],
    )
    .await?;

    let mut rows = conn.query("SELECT last_insert_rowid()", params![]).await?;
    let row = rows
        .next()
        .await?
        .context("failed to get last insert rowid")?;
    let id = row.get::<i64>(0)?;

    Ok(id)
}

#[allow(clippy::missing_panics_doc)]
pub async fn insert_chunk(
    conn: &Connection,
    document_id: i64,
    chunk_index: usize,
    content: &str,
    embedding: &[f32],
) -> anyhow::Result<()> {
    let embedding_json = serde_json::to_string(embedding)?;
    conn.execute(
        "INSERT INTO chunks (document_id, chunk_index, content, embedding) \
         VALUES (?1, ?2, ?3, vector32(?4))",
        params![
            document_id,
            i64::try_from(chunk_index).expect("chunk index fits i64"),
            content,
            embedding_json
        ],
    )
    .await?;

    Ok(())
}

#[derive(Debug)]
pub struct SearchHit {
    pub score: f64,
    pub content: String,
    pub doc_title: String,
    pub doc_path: String,
}

pub async fn vector_search(
    conn: &Connection,
    query_embedding: &[f32],
    top_k: usize,
) -> anyhow::Result<Vec<SearchHit>> {
    let embedding_json = serde_json::to_string(query_embedding)?;
    let sql = format!(
        "SELECT c.content, d.title, d.source_path \
         FROM vector_top_k('idx_chunks_embedding', vector32(?1), {top_k}) AS v \
         JOIN chunks AS c ON c.rowid = v.id \
         JOIN documents AS d ON d.id = c.document_id"
    );
    let mut rows = conn.query(&sql, params![embedding_json]).await?;

    let mut results = Vec::new();
    let mut rank = 0_u32;
    let total = u32::try_from(top_k).unwrap_or(u32::MAX);
    while let Some(row) = rows.next().await? {
        let content = row.get::<String>(0)?;
        let doc_title = row.get::<String>(1)?;
        let doc_path = row.get::<String>(2)?;
        let score = 1.0 - f64::from(rank) / f64::from(total);
        results.push(SearchHit {
            score,
            content,
            doc_title,
            doc_path,
        });
        rank += 1;
    }

    Ok(results)
}
