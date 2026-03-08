+++
title = "💎  Part 4: The Pearl Forge — Embeddings and Vector Storage"
description = "Generating embeddings with Ollama and storing vectors in libSQL"
date = 2026-03-11
[taxonomies]
tags = ["rust", "rag", "embeddings", "libsql"]
[extra]
illustration = "img/part4-pearls.jpg"
photo_credit = "Marin Tulard"
photo_credit_url = "https://unsplash.com/@mtulard"
+++

*The Crab Island RAG Expedition — Part 4 of 6*

---

Deep in the heart of Crab Island, past the coconut groves and through the bamboo thicket, lies the Pearl Forge.

This is where the real magic happens. The crabs bring their coconut pieces here, and the Forge transforms each one into a **pearl** — a shimmering 768-faceted gem that encodes the *meaning* of the text. Two pieces about similar topics produce pearls that glow in similar ways. Two pieces about unrelated topics produce pearls that look completely different.

And the beach? It's no ordinary beach. It's a **smart beach** — powered by libsql — that remembers where every pearl is buried and can instantly find the ones most similar to any new pearl you show it.

SQLite learned to understand meaning. Let's see how.

## How the Pearl Forge Works

An embedding is a list of numbers (a vector) that captures the meaning of text. "How do I handle errors in Rust?" and "What's the best way to manage failures?" produce vectors that are close together in 768-dimensional space, even though they share almost no words.

The Forge is powered by `nomic-embed-text` running locally via Ollama. Here's how we operate it:

```rust
// src/embed.rs
use rig::client::EmbeddingsClient;
use rig::embeddings::EmbeddingModel as _;
use rig::providers::ollama;

const NOMIC_EMBED_TEXT: &str = "nomic-embed-text";
pub const EMBEDDING_DIMS: usize = 768;

#[must_use]
pub fn embedding_model(client: &ollama::Client) -> ollama::EmbeddingModel {
    client.embedding_model_with_ndims(NOMIC_EMBED_TEXT, EMBEDDING_DIMS)
}

pub async fn embed_text(
    model: &ollama::EmbeddingModel,
    text: &str,
) -> anyhow::Result<Vec<f32>> {
    let embedding = model.embed_text(text).await?;
    Ok(to_f32(&embedding.vec))
}

pub async fn embed_texts(
    model: &ollama::EmbeddingModel,
    texts: impl IntoIterator<Item = String> + Send,
) -> anyhow::Result<Vec<Vec<f32>>> {
    let embeddings = model.embed_texts(texts).await?;
    Ok(embeddings.iter().map(|e| to_f32(&e.vec)).collect())
}

// rig returns Vec<f64>, libsql F32_BLOB needs f32
#[allow(clippy::cast_possible_truncation)]
fn to_f32(vec: &[f64]) -> Vec<f32> {
    vec.iter().map(|&v| v as f32).collect()
}
```

Two functions: `embed_text` for single queries (the chat side), `embed_texts` for batch ingestion (the sync side — we'll use this in Part 6 to speed things up). Both share a `to_f32` helper that handles the f64→f32 conversion.

Wait. Did you notice that suspicious cast in `to_f32`?

## The Reef of Mismatched Gems

Every island has a hidden reef, and this is ours — the **f64/f32 mismatch**.

- **rig-core** produces pearls with 64-bit precision (`Vec<f64>`)
- **libsql** stores pearls with 32-bit precision (`F32_BLOB`)

So every time the Forge creates a pearl, we have to file it down from f64 to f32. And every time we search, same thing. The precision loss is negligible for similarity search (pearl facets are typically between -1.0 and 1.0), but Clippy — the island's quality inspector — flags it immediately.

We handle it with `#[allow(clippy::cast_possible_truncation)]`. This is one of those cases where the cast is intentional, the inspector is noted, and life goes on.

> **Expedition tip**: You could store pearls in full f64 precision, but libsql's vector functions are optimized for f32. The storage savings are real too: 768 dimensions × 4 bytes = 3KB per pearl vs. 6KB. With thousands of pearls buried on the beach, that adds up to a lighter island.

**Other Pearl Forge settings:**

| Model | Facets (dims) | Notes |
|-------|---------------|-------|
| `nomic-embed-text` | 768 | Our pick — good balance of quality and size |
| `mxbai-embed-large` | 1024 | More facets, higher fidelity |
| `all-minilm` | 384 | Fewer facets, faster forging |
| `snowflake-arctic-embed` | 1024 | Great for larger pieces of text |

Switching is easy — just change `NOMIC_EMBED_TEXT` and `EMBEDDING_DIMS`. The rest of the expedition doesn't care which Forge setting you use.

## The Smart Beach

Now let's design our beach. Two tables, one index:

```rust
// src/db.rs
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
```

Let's explore the beach:

**`documents` table**: The registry — which treasures have we processed? The `mtime` column is clever: it records *when* the source file was last modified, so we can skip treasures we've already processed.

**`chunks` table**: The pearl grid — each pearl has its content (the original coconut piece text) and its embedding (`F32_BLOB(768)` — 768 facets of 32-bit shimmer).

**`libsql_vector_idx(embedding, 'metric=cosine')`**: This is the beach's magic — a cosine similarity index. When a crab dives looking for pearls, this index tells it which direction to swim. Cosine similarity measures the *angle* between two pearl facet patterns — the standard metric for text embeddings.

The whole beach lives in a single file. No server, no infrastructure, no cloud. Just a `.db` file on your island.

## Burying Pearls

```rust
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
```

Notice `.context("failed to get last insert rowid")?` — anyhow's way of turning an `Option::None` into a descriptive error. Much cleaner than constructing a `libsql::Error` by hand.

And burying each pearl — notice how the embedding gets JSON-serialized and passed through `vector32()`:

```rust
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
```

`vector32(?4)` is libsql's pearl press — it takes a JSON array of floats (`[0.123, -0.456, ...]`) and compresses it into a binary vector blob. Serialize with serde, pass as text, let the beach handle the rest.

## The Clever Crabs: Idempotent Ingestion

No crab wants to re-forge 758 pearls for a coconut they already processed yesterday. We use file modification time (`mtime`) for cheap change detection:

```rust
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

pub async fn delete_document(
    conn: &Connection,
    doc_id: i64,
) -> anyhow::Result<()> {
    conn.execute("DELETE FROM chunks WHERE document_id = ?1", params![doc_id])
        .await?;
    conn.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
        .await?;
    Ok(())
}
```

The crab's decision tree:
1. "Have I seen this bottle before?" → Check `find_document_by_path`
2. Same mtime? → "Already processed, moving on!" (skip)
3. Different mtime? → "Bottle was refilled!" → dig up old pearls, forge new ones
4. Never seen it? → "Fresh treasure!" → forge and bury

> **Expedition tip**: For a permanent settlement, you might use content hashing (SHA256) instead of mtime — it's more reliable across file systems and backups. But for our expedition, mtime is simple, fast, and does the job.

> **Expedition tip — schema migrations**: Our `CREATE TABLE IF NOT EXISTS` works fine for a fresh expedition. But a real settlement would need proper schema migration — adding columns, renaming tables, evolving the beach layout over time. Tools like [refinery](https://crates.io/crates/refinery) or [sqlx-migrate](https://crates.io/crates/sqlx-migrate) handle this elegantly. For our adventure, we just delete the database and re-sync when the schema changes.

> **Note — the `SmallVec` temptation**: If you're a Rust performance enthusiast, you might think: "768 elements, always the same size — perfect for `SmallVec<[f32; 768]>` to avoid heap allocation!" Resist! A `SmallVec` with `N=768` puts 3KB on the stack (`768 × 4 bytes`). That's unusually large for a stack allocation, and in practice `SmallVec` will spill to the heap anyway once you pass it around. `SmallVec` shines for *tiny* collections (1–8 elements) where avoiding a heap allocation actually matters. For embeddings, a plain `Vec<f32>` is simpler and equally fast. Don't fight the allocator when it's already doing a great job.

## Beach Report

The Pearl Forge and Smart Beach are operational:

- **Embedding generation**: text → 768-faceted pearl, forged locally via Ollama
- **Database schema**: two tables with a cosine similarity index
- **CRUD operations**: register treasures, bury pearls, dig up old ones
- **Idempotent ingestion**: clever crabs skip what they've already processed

The pearls are buried. Now we need to teach the crabs how to *find* the right ones. In the next post, we implement vector search — the diving technique that lets crabs find pearls by meaning.
