+++
title = "🏕️  Part 1: Landfall — Setting Up Camp on Crab Island"
description = "Project setup, dependencies, and why Rust for RAG"
date = 2026-03-08
[taxonomies]
tags = ["rust", "rag"]
[extra]
illustration = "img/part1-camp.jpg"
photo_credit = "Mario Am\u00e9"
photo_credit_url = "https://unsplash.com/@imperioame"
+++

*The Crab Island RAG Expedition — Part 1 of 6*

---

Welcome, adventurer, to Crab Island.

This is a lush tropical paradise — dense jungle canopy, white sand beaches, crystal-clear lagoons. The locals are crabs. Resourceful, industrious, type-safe crabs. And here's the best part: **there are absolutely no snakes on this island**. Not a single one. The crabs made sure of that ages ago.

You've arrived here because you've heard rumors of ancient knowledge hidden across the island — treasure maps carved on coconut shells, messages sealed in bottles, scrolls tucked inside coral caves. You want to find that knowledge and *ask questions* about it. But the island is vast, and there's no way to read everything yourself.

Fortunately, the crabs have a system. They can crack open any container, break the contents into pearl-sized pieces, bury those pearls in special spots on the beach, and later dive to find exactly the right ones when you have a question. They even have a wise old parrot who can synthesize answers from whatever pearls the crabs bring back.

This, dear adventurer, is a RAG system. And we're going to build it in Rust.

## The Expedition Plan

RAG (Retrieval-Augmented Generation) is just a treasure hunt with six steps:

1. **Extract** — Open the bottles, unroll the scrolls, decipher the maps (get text from PDFs, EPUBs, etc.)
2. **Chunk** — Crack the coconuts into bite-sized pieces (split text into small passages)
3. **Embed** — Translate each piece into the island's musical language (convert text to numerical vectors)
4. **Store** — Bury each pearl in a labeled spot on the beach (save vectors in a database)
5. **Retrieve** — Send crabs diving for the pearls that match your question (vector similarity search)
6. **Generate** — Bring the pearls to the wise parrot, who weaves them into an answer (LLM generation)

```
PDF/EPUB/HTML → extract → chunks → embed → store (vector DB)
                                                     ↓
              query → embed → similarity search → LLM → answer
```

## Why This Island? (Why Rust?)

The Python continent across the sea is wonderful — bustling ports, established trade routes, a bazaar with every tool imaginable (LangChain, LlamaIndex, ChromaDB). If you need something built fast with maximum community support, sail there. No judgment. The Python ecosystem for AI is genuinely incredible.

But Crab Island offers something different:

- **Type-safe pipelines**: Wire things wrong and the island's guardian (the compiler) stops you *before* you waste an afternoon embedding the wrong data.
- **Error handling that doesn't lie**: No surprise `NoneType has no attribute 'content'` erupting from a swamp at 2am. Every failure path is mapped.
- **Single-artifact deployment**: `cargo build --release` produces one executable. Carry it anywhere. No virtualenv, no `requirements.txt`.
- **Async by default**: Tokio keeps our I/O humming while crabs work in parallel across the beach.
- **No GIL**: When we need the crabs to truly work simultaneously, nothing holds them back.

## The Expedition Gear

Four crates form our toolkit — each one a trusty piece of island equipment:

| Crate | Role | Island Metaphor |
|-------|------|-----------------|
| **rig-core** | LLM abstraction | The wise parrot's translation manual |
| **libsql** | Database + vectors | The beach where we bury and find pearls |
| **kreuzberg** | Document extraction | The bottle opener / scroll unfurler |
| **Ollama** | Local LLM runtime | The parrot itself, living on your island |

Note that Ollama is external software (the LLM server), not a Rust crate — rig-core provides the Rust interface to it.

**Other gear you could pack:**

- Instead of **rig-core**, you could use [llm-chain](https://github.com/sobelio/llm-chain) or call Ollama's REST API by hand. rig-core wins because it provides clean abstractions for both embeddings and completions, with built-in Ollama support.
- Instead of **libsql**, you could bring a [qdrant](https://github.com/qdrant/qdrant-client) or [lancedb](https://github.com/lancedb/lancedb) server. We chose libsql because it's just a single file on the beach — no infrastructure, no server process, and it handles both relational data *and* vector search.
- Instead of **kreuzberg**, you could use [pdf-extract](https://crates.io/crates/pdf-extract) for maps (PDFs) only. kreuzberg handles 75+ container types through a single function — bottles, scrolls, coconut carvings, you name it.
- Instead of **Ollama**, you could summon a cloud parrot (OpenAI, Anthropic). We keep ours local so the entire expedition stays on the island.

## Setting Up Base Camp

Our camp has a library (shared knowledge) and two outposts — one for gathering treasures, one for asking questions:

```toml
# Cargo.toml
[package]
name = "mini-rag"
version = "0.1.0"
edition = "2024"
rust-version = "1.91.1"

[lib]
name = "mini_rag"
path = "src/lib.rs"

[[bin]]
name = "rag-sync"
path = "src/bin/sync.rs"

[[bin]]
name = "rag-chat"
path = "src/bin/chat.rs"

[dependencies]
anyhow = "1"
kreuzberg = { version = "4.5.1", features = ["pdf"] }
kreuzberg-pdfium-render = "4.5.1" # pin to match kreuzberg 4.5.x (upstream version mismatch)
libsql = "0.9.30"
owo-colors = "4"
rig-core = "0.33.0"
serde_json = "1.0.149"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tracing = "0.1.44"
tracing-subscriber = "0.3.23"
```

> **Note**: This project requires Rust 1.91.1+ (due to dependencies MSRV). Check with `rustc --version`.

The library holds all shared modules, plus a few helpers that both binaries need:

```rust
// src/lib.rs
use anyhow::Context;
use rig::providers::ollama;

pub mod chunk;
pub mod db;
pub mod embed;
pub mod extract;
pub mod rag;

pub const DB_PATH: &str = "mini_rag.db";
pub const DEEPSEEK_R1: &str = "deepseek-r1";

pub fn ollama_client() -> anyhow::Result<ollama::Client> {
    ollama::Client::new(rig::client::Nothing)
        .context("failed to create Ollama client")
}
```

Five modules for the core logic, a shared database path, the model name, and a helper to create the Ollama client. Both binaries import from here instead of duplicating setup code.

## The Island's Danger Log

Before we venture into the jungle, we need a way to record what goes wrong. On the Python continent, people sometimes use bare `except:` blocks and hope for the best. On Crab Island, every `?` operator automatically captures and propagates errors with full context.

We use [`anyhow`](https://crates.io/crates/anyhow) — a pragmatic error handling crate that lets us write `anyhow::Result<T>` everywhere and attach human-readable context with `.context("what went wrong")`:

```rust
// No error.rs needed — anyhow handles it all
pub fn ollama_client() -> anyhow::Result<ollama::Client> {
    ollama::Client::new(rig::client::Nothing)
        .context("failed to create Ollama client")
}
```

The `?` operator works with any error type that implements `std::error::Error` — `io::Error`, `libsql::Error`, `kreuzberg::KreuzbergError` — they all flow through `anyhow` automatically. No boilerplate, no manual `From` impls. The danger log stays short and sweet.

> **Going further — typed error enums**: For a library where callers need to `match` on specific error variants (retry on network error, abort on auth error), consider [`thiserror`](https://crates.io/crates/thiserror) or [`derive_more`](https://crates.io/crates/derive_more) to define a custom error enum. For our expedition — a PoC where we only display errors, never match on them — `anyhow` is the right tool.

## Training the Parrot

Before the expedition can begin, our parrot needs two skills:

```bash
# Understanding language (converting text to meaning-vectors)
ollama pull nomic-embed-text

# Speaking wisdom (generating answers from context)
ollama pull deepseek-r1
```

`nomic-embed-text` gives the crabs their sense of direction — it converts text into 768-dimensional coordinates so they know where to bury and find pearls. Alternatives like `mxbai-embed-large` (1024 dims) or `all-minilm` (384 dims) work too — just different maps of the same beach.

`deepseek-r1` is our parrot — a reasoning model that "thinks out loud" in `<think>` tags before speaking. We'll need to clean up its muttering later (spoiler for Part 6!), but the wisdom it delivers is excellent.

## The Expedition Ahead

Here's our route across the island:

- **Part 2**: Open the bottles and unroll the scrolls — text extraction (it's 3 lines, seriously)
- **Part 3**: Crack coconuts into pieces — the art of chunking
- **Part 4**: Turn pieces into pearls and bury them on the beach — embeddings and vector storage
- **Part 5**: Train the crabs to dive for the right pearls — vector search
- **Part 6**: Ask the parrot — wire it all into sync and chat binaries

By Part 6, we'll have a fully working, snake-free RAG system. Grab your machete and let's head into the jungle.
