+++
title = "🤿  Part 5: The Pearl Divers — Implementing Vector Search"
description = "Implementing vector similarity search with SQL and cosine distance"
date = 2026-03-12
[taxonomies]
tags = ["rust", "rag", "vector-search"]
[extra]
illustration = "img/part5-diving.jpg"
photo_credit = "John Cameron"
photo_credit_url = "https://unsplash.com/@john_cameron"
+++

*The Crab Island RAG Expedition — Part 5 of 6*

---

The beach is full of buried pearls. Hundreds of them — 758 from the Comprehensive Rust scroll alone, each one a shimmering 768-faceted gem encoding a coconut piece's meaning.

Now comes the critical question: when someone asks "What is Rust's ownership model?", how do the crabs find the *right* pearls?

They can't dig up every single one and compare. That would take forever. Instead, they need a diving technique — a way to take the question, forge a question-pearl from it, and then dive straight to the pearls that glow most similarly.

About 30 lines. And once we write them, the expedition is nearly complete.

## The Diving Technique

The dive has three steps:

1. **Forge a question-pearl** — embed the user's question using the same Forge (nomic-embed-text)
2. **Dive the beach** — ask libsql's `vector_top_k` for the closest pearls
3. **Surface with results** — return the matching text chunks with source metadata

Here's our search function:

```rust
// src/db.rs (continued)

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
```

A few things to unpack.

## The JOIN: Pearls with Provenance

Notice the `JOIN documents` — we fetch not just the chunk content, but also which document it came from (`title`, `source_path`). This is critical for **source attribution**: when the parrot answers a question, the explorer can see *where* the answer came from.

The `SearchHit` struct carries everything we need: the matching text, a relevance score, and the document metadata.

## Reef #1: The Beach Doesn't Tell Distance

This one puzzled us for a while. libsql's `vector_top_k()` returns results ordered by similarity — the closest pearls first. Great! But it **does not tell you *how close* they are**. There's no distance column. No similarity score.

If you try `SELECT v.distance FROM vector_top_k(...)`:

```
SqliteFailure: no such column: distance
```

The beach knows the order, but keeps the measurements to itself.

Our workaround: **rank-based scoring**.

```rust
let score = 1.0 - f64::from(rank) / f64::from(total);
```

With `top_k = 5`: first pearl scores `1.0`, second `0.8`, third `0.6`, fourth `0.4`, fifth `0.2`. The actual numbers are cosmetic — our results are already in the right order. The crabs find the right pearls regardless.

> **Expedition tip**: If you need true distance scores, consider [usearch](https://crates.io/crates/usearch) — an embeddable single-header HNSW library with Rust bindings. Like libsql, it's in-process (no server), but it returns actual cosine distances and supports custom metrics. For a full-blown search server, [qdrant](https://crates.io/crates/qdrant-client) or [lancedb](https://crates.io/crates/lancedb) are solid options. We chose our Smart Beach (libsql) for its simplicity — one file handles both storage and search — and rank-based scoring is an acceptable trade-off.

> **Reef warning — no rejection of irrelevant pearls**: Without actual distance scores, we can't tell whether a result is genuinely close or just the *least far away*. Ask "What's a good pasta recipe?" and the crabs will still surface 5 pearls — they'll just be the least irrelevant Rust chunks. A production system would want a similarity threshold to reject results that are too far from the question. With libsql's `vector_top_k`, that's not possible today. With usearch, qdrant, or lancedb, you'd filter on `score > 0.7` (or whatever threshold fits your data). Something to keep in mind if your treasure hoard might receive off-topic questions.

## Reef #2: `top_k` Can't Be Parameterized

Notice `{top_k}` is interpolated into the SQL string, not passed as `?2`. That's because `vector_top_k`'s second argument must be a literal integer in the SQL — it won't accept a parameter. Since `top_k` is always a `usize` from our own code (never user input), this is safe. But it's another reef to be aware of.

## Wiring It Into the RAG Query

With `vector_search` in hand, our RAG module can stay refreshingly simple — no traits, no generics, just a direct pipeline:

```rust
// src/rag.rs
use libsql::Connection;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::ollama;

use crate::db;
use crate::embed;
use crate::DEEPSEEK_R1;

const TOP_K: usize = 5;

pub async fn query(
    client: &ollama::Client,
    embedding_model: &ollama::EmbeddingModel,
    conn: &Connection,
    question: &str,
) -> anyhow::Result<RagResponse> {
    // 1. Forge a question-pearl
    let query_embedding = embed::embed_text(embedding_model, question).await?;

    // 2. Dive for the top 5 pearls
    let hits = db::vector_search(conn, &query_embedding, TOP_K).await?;

    // 3. Build context and collect sources in one pass
    let mut context = String::new();
    let mut sources = Vec::with_capacity(hits.len());
    for (i, hit) in hits.into_iter().enumerate() {
        if !context.is_empty() {
            context.push_str("\n\n");
        }
        let _ = write!(context, "[{}] {}", i + 1, &hit.content);
        sources.push(Source {
            excerpt: truncate_to_excerpt(&hit.content, EXCERPT_LEN),
            title: hit.doc_title,
            path: hit.doc_path,
        });
    }

    // 4. Ask the parrot with explicit context
    let agent = client
        .agent(DEEPSEEK_R1)
        .preamble(
            "You are Kwaak 🦜, the wise parrot of Crab Island. \
             You speak with parrot flair — sprinkle in the occasional \
             *squawk!*, *BRAWWK!*, or *ruffles feathers*, and use \
             emojis like 🦀🥥🏝️. But under the plumage, you are \
             precise and knowledgeable. \
             Use the provided context to answer accurately. If the \
             context doesn't contain enough information, squawk \
             honestly — never make up answers. Keep answers concise.",
        )
        .build();

    let prompt = format!("Context:\n{context}\n\nQuestion: {question}");
    let response = agent.prompt(prompt).await?;

    Ok(RagResponse {
        answer: strip_think_tags(&response),
        sources,
    })
}
```

No `VectorStoreIndex` trait, no `dynamic_context`, no `serde::Deserialize` gymnastics. Just: embed the question → search the beach → build a prompt → ask the parrot. Four steps, all explicit, all debuggable.

> **Going further — rig's `dynamic_context`**: rig-core has a `VectorStoreIndex` trait and a `dynamic_context` builder method that automates the embed-search-inject pipeline. You implement the trait for your database, and rig handles the orchestration. It's elegant when you have multiple vector stores or want rig to manage the context window. But it requires implementing two async trait methods with `serde::Deserialize` bounds, and for our PoC the direct approach is simpler, more transparent, and gives us full control over source attribution. The trait earns its keep in larger systems — start direct, add the abstraction when you need it.

## Alternative Diving Techniques

Our `vector_search` is purpose-built for the Smart Beach. Other options:

- **[usearch](https://crates.io/crates/usearch)**: An embeddable HNSW index with Rust bindings. In-process like libsql, but returns actual distances and supports custom metrics. A good middle ground between our simple approach and a full search server.
- **rig's built-in integrations**: rig-core has optional support for MongoDB, LanceDB, Neo4j, and more. Check the [rig docs](https://docs.rs/rig-core) — someone may have already built a diving crew for your preferred beach.
- **In-memory diving**: For small treasure hoards, skip the beach entirely. Compute cosine similarity over a `Vec<Vec<f32>>` in memory. Fast for development, won't scale.
- **Hybrid search**: Combine pearl diving (vector search) with word matching (FTS5 in SQLite). Use vector search for meaning and FTS for exact keywords. libsql supports both on the same beach.

## Beach Report

In ~30 lines of search code plus ~40 lines of RAG wiring, our diving crew can:

- Take any question and forge a question-pearl
- Navigate the f64/f32 reef at the boundary
- Dive the Smart Beach using `vector_top_k` with source metadata
- Surface with ranked results including document provenance
- Build explicit context for the parrot — no magic, full control

In the final post, we bring the whole expedition together. Two binaries, one database, and a conversation with our Rust books.
