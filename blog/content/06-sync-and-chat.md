+++
title = "🦜  Part 6: Asking the Parrot — The Full RAG Pipeline"
description = "Wiring sync and chat binaries into a full RAG pipeline"
date = 2026-03-13
[taxonomies]
tags = ["rust", "rag", "ollama"]
[extra]
illustration = "img/part6-parrot.jpg"
photo_credit = "Morty Smith"
photo_credit_url = "https://unsplash.com/@mortypics"
illustration_position = "center 50%"
+++

*The Crab Island RAG Expedition — Part 6 of 6*

---

The sun is high over Crab Island. The expedition has covered a lot of ground — we've opened bottles, cracked coconuts, forged pearls, buried them on the Smart Beach, and trained the crabs to dive.

Now it's time for the grand finale: we connect everything together and actually *talk to our documents*. Two outposts, one beach, and a very wise parrot.

Picture the scene: you walk up to the parrot's perch overlooking the lagoon, and ask a question. The parrot squawks at the crabs. They dive, surface with a clawful of relevant pearls, and lay them at the parrot's feet. The parrot studies them, ruffles its feathers, and speaks wisdom grounded in your actual documents.

That's RAG. That's what we're building. Let's wire it up.

## The Two Outposts

Our island has two outposts sharing the same Smart Beach:

```
┌─────────────┐     ┌──────────────────────────┐     ┌─────────────┐
│  rag-sync   │────→│       mini_rag.db        │←────│  rag-chat   │
│ (gathering) │     │  treasures + pearls +    │     │ (the perch) │
│             │     │  vector embeddings       │     │             │
└─────────────┘     └──────────────────────────┘     └─────────────┘
       │                                                     │
       ▼                                                     ▼
  open → crack → forge → bury              question → dive → parrot → answer
```

`rag-sync` sends the crabs out to gather and process treasures. `rag-chat` is the parrot's perch where you ask questions. They share the beach database but never need to run at the same time.

## Shared Camp Gear

Before we look at the outposts, both binaries share some equipment from the library:

```rust
// src/lib.rs
pub mod chunk;
pub mod db;
pub mod embed;
pub mod extract;
pub mod rag;

use anyhow::Context;
use rig::providers::ollama;

pub const DB_PATH: &str = "mini_rag.db";
pub const DEEPSEEK_R1: &str = "deepseek-r1";

pub fn ollama_client() -> anyhow::Result<ollama::Client> {
    ollama::Client::new(rig::client::Nothing)
        .context("failed to create Ollama client")
}
```

The `DB_PATH` and `DEEPSEEK_R1` constants live here so both binaries use the same values. The `ollama_client()` helper avoids duplicating the client creation logic — DRY, even on a tropical island.

## The Gathering Outpost

This is the expedition's workhorse. Hand it file paths or directories, and the crabs open every container, crack the contents, forge pearls, and bury them — skipping anything they've already processed.

```rust
// src/bin/sync.rs
#![allow(clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Context;
use rig::providers::ollama;
use mini_rag::chunk;
use mini_rag::{DB_PATH, db, embed, extract};

const CHUNK_SIZE: usize = 500;
const CHUNK_OVERLAP: usize = 50;
```

First, a way to read a bottle's wax seal date (file modification time):

```rust
fn file_mtime(path: &Path) -> anyhow::Result<i64> {
    let metadata = std::fs::metadata(path)?;
    let mtime = metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system time before UNIX epoch")?;
    #[allow(clippy::cast_possible_wrap)]
    Ok(mtime.as_secs() as i64)
}
```

Notice how `anyhow`'s `.context()` replaces what would otherwise be a manual `map_err` with an `io::Error` — cleaner and more descriptive.

> **Portability note**: `metadata.modified()` works on all major platforms (Linux, macOS, Windows) but can return `Err` on exotic targets. The `?` handles that gracefully. A subtler issue: mtime can be unreliable on network filesystems (NFS clock skew) or after copying files across machines. For a production system, content hashing (e.g. SHA-256) would be more robust — but for a local expedition, mtime is simple and fast enough.

Then, scouting the jungle for treasures — no extra dependency, just `std::fs::read_dir`:

```rust
fn collect_files(args: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for arg in args {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            walk_dir(&path, &mut files)?;
        } else if path.is_file() {
            files.push(path);
        } else {
            tracing::warn!(
                "Skipping {}: not a file or directory",
                path.display(),
            );
        }
    }
    Ok(files)
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}
```

> **Expedition tip**: For a permanent settlement, consider [walkdir](https://crates.io/crates/walkdir) or [ignore](https://crates.io/crates/ignore) (the crate powering ripgrep). They handle symlinks, permission errors, and `.gitignore` rules gracefully. Our manual scouting works fine for the expedition.

Now the heart of the operation — processing a single treasure:

```rust
fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .map_or_else(
            || "Untitled".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        )
}

async fn ingest_file(
    conn: &libsql::Connection,
    embedding_model: &ollama::EmbeddingModel,
    path: &Path,
) -> anyhow::Result<()> {
    let source_path = path.to_string_lossy();
    let mtime = file_mtime(path)?;

    // The clever crab check: have we seen this before?
    if let Some((existing_id, existing_mtime)) =
        db::find_document_by_path(conn, &source_path).await?
    {
        if existing_mtime == mtime {
            tracing::info!("Skipping {} (unchanged)", path.display());
            return Ok(());
        }
        tracing::info!(
            "Re-ingesting {} (mtime changed)",
            path.display(),
        );
        db::delete_document(conn, existing_id).await?;
    }

    tracing::info!("Ingesting {}...", path.display());

    // The full pipeline: open → crack → forge → bury
    let content = extract::extract_text(path).await?;
    tracing::info!(
        "Extracted {} characters from {}",
        content.len(),
        path.display(),
    );

    let title = title_from_path(path);
    let doc_id = db::insert_document(
        conn, &title, &source_path, mtime,
    ).await?;

    let chunks = chunk::chunk_text(&content, CHUNK_SIZE, CHUNK_OVERLAP);
    tracing::info!(
        "Created {} chunks for {}",
        chunks.len(),
        path.display(),
    );

    for (i, c) in chunks.iter().enumerate() {
        let embedding = embed::embed_text(
            embedding_model, &c.content,
        ).await?;
        db::insert_chunk(
            conn, doc_id, i, &c.content, &embedding,
        ).await?;
        if i % 10 == 0 {
            tracing::info!(
                "Embedded chunk {}/{}",
                i + 1,
                chunks.len(),
            );
        }
    }

    tracing::info!("Done ingesting {}", path.display());
    Ok(())
}
```

And the main function — the expedition leader:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: rag-sync <file-or-dir>...");
        std::process::exit(1);
    }

    let files = collect_files(&args)?;
    if files.is_empty() {
        tracing::warn!("No files found to ingest");
        return Ok(());
    }
    tracing::info!("Found {} file(s) to process", files.len());

    let conn = db::init_db(Path::new(DB_PATH)).await?;
    let client = mini_rag::ollama_client()?;
    let embedding_model = embed::embedding_model(&client);

    for file in &files {
        if let Err(e) = ingest_file(
            &conn, &embedding_model, file,
        ).await {
            tracing::error!(
                "Failed to ingest {}: {e}",
                file.display(),
            );
        }
    }

    Ok(())
}
```

Note `mini_rag::ollama_client()?` — the shared helper from `lib.rs`. Both binaries create the Ollama client the same way.

Let's send the crabs to work on our Rust treasures:

```bash
$ cargo run --bin rag-sync -- comprehensive-rust.pdf rust-design-patterns.pdf

INFO Ingesting comprehensive-rust.pdf...
INFO Extracted 348572 characters from comprehensive-rust.pdf
INFO Created 758 chunks for comprehensive-rust.pdf
INFO Embedded chunk 1/758
INFO Embedded chunk 11/758
...
INFO Done ingesting comprehensive-rust.pdf
INFO Ingesting rust-design-patterns.pdf...
INFO Extracted 85431 characters from rust-design-patterns.pdf
INFO Created 187 chunks for rust-design-patterns.pdf
...
INFO Done ingesting rust-design-patterns.pdf
```

Run it again — the crabs are too smart to repeat work:

```bash
$ cargo run --bin rag-sync -- comprehensive-rust.pdf rust-design-patterns.pdf

INFO Skipping comprehensive-rust.pdf (unchanged)
INFO Skipping rust-design-patterns.pdf (unchanged)
```

Idempotent crabs. The best kind.

## Speeding Up the Crabs

The simple loop works, but it's slow — 758 chunks means 758 individual HTTP requests to Ollama and 758 individual database commits. On a real beach, that takes about 2 minutes. Three quick wins can dramatically cut that time.

**Win #1: Batch embeddings.** Ollama's embedding API accepts multiple texts in a single request. Instead of 758 HTTP round-trips, we send batches of 200 — just 4 requests:

```rust
// src/embed.rs — add a batch variant
pub async fn embed_texts(
    model: &ollama::EmbeddingModel,
    texts: impl IntoIterator<Item = String> + Send,
) -> anyhow::Result<Vec<Vec<f32>>> {
    let embeddings = model.embed_texts(texts).await?;
    Ok(embeddings.iter().map(|e| to_f32(&e.vec)).collect())
}
```

Under the hood, rig-core's `embed_texts` sends all texts in a single `"input": [...]` JSON payload. The GPU processes them as a batch — much faster than one at a time.

**Win #2: Database transaction.** Without an explicit transaction, each `INSERT` auto-commits — that's 758 fsyncs to disk. Wrapping them in a single transaction means one fsync at the end:

```rust
let tx = conn.transaction().await?;
// ... all inserts go through &tx ...
tx.commit().await?;
```

Putting it all together — batch embedding with transactional inserts:

```rust
const EMBED_BATCH_SIZE: usize = 200;

// In ingest_file:
let tx = conn.transaction().await?;
let mut chunk_idx = 0;
for batch in chunks.chunks(EMBED_BATCH_SIZE) {
    let texts: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
    let embeddings = embed::embed_texts(embedding_model, texts).await?;

    for (chunk, embedding) in batch.iter().zip(embeddings) {
        db::insert_chunk(&tx, doc_id, chunk_idx, &chunk.content, &embedding).await?;
        chunk_idx += 1;
    }

    tracing::info!("Embedded {}/{} chunks", chunk_idx, chunks.len());
}
tx.commit().await?;
```

The biggest win is batch embedding — it eliminates per-chunk HTTP overhead. The transaction groups all disk writes into a single flush at commit.

> **Why not rayon?** The bottleneck is I/O (HTTP to Ollama + DB writes), not CPU. Rayon shines for CPU-bound parallelism — chunking, parsing, hashing. Here, `tokio` is the right tool because we're dealing with async I/O.

> **Why not concurrent files?** Ollama serializes GPU inference — sending embedding requests from two files simultaneously just queues them. Concurrent files would only help overlap *extraction* of file B with *embedding* of file A, which is marginal.

<details>
<summary><strong>Going further — pipelining with tokio::spawn</strong></summary>

For larger datasets, you could overlap embedding batch N+1 with inserting batch N's results. The idea: spawn DB inserts on a background task while the next embedding call is in flight.

```rust
let mut chunk_idx = 0;
let mut pending_insert: Option<tokio::task::JoinHandle<anyhow::Result<()>>> = None;

for batch in chunks.chunks(EMBED_BATCH_SIZE) {
    let texts: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
    let embeddings = embed::embed_texts(embedding_model, texts).await?;

    if let Some(handle) = pending_insert.take() {
        handle.await??;
    }

    let tx_clone = tx.clone();
    let batch_data: Vec<_> = batch.iter().zip(embeddings)
        .map(|(c, emb)| { chunk_idx += 1; (chunk_idx - 1, c.content.clone(), emb) })
        .collect();

    pending_insert = Some(tokio::spawn(async move {
        for (idx, content, emb) in &batch_data {
            db::insert_chunk(&tx_clone, doc_id, *idx, content, emb).await?;
        }
        Ok(())
    }));
}

if let Some(handle) = pending_insert.take() {
    handle.await??;
}
```

In practice, DB inserts are fast relative to embedding, so the gain is small. The simpler sequential version above is what we actually ship — add the pipeline when profiling tells you to.

</details>

## The Parrot's Brain

We already built the `query()` function in Part 5 — embed the question, dive the beach, build context, ask the parrot. Here we add the finishing touches: structured responses with source attribution, and pretty-printing for the terminal.

The response includes the answer *and* where it came from:

```rust
// src/rag.rs
#[derive(Debug)]
pub struct Source {
    pub title: String,
    pub path: String,
    pub excerpt: String,
}

#[derive(Debug)]
pub struct RagResponse {
    pub answer: String,
    pub sources: Vec<Source>,
}
```

The sources get pretty-printed with colored output via `owo-colors`:

```rust
impl fmt::Display for RagResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.answer)?;

        if !self.sources.is_empty() {
            writeln!(f, "\n{}", "─── Sources ───".dimmed())?;
            for (i, source) in self.sources.iter().enumerate() {
                let idx = format!("[{}]", i + 1);
                let location = format!(
                    "{} ({})", source.title.green(), source.path.dimmed()
                );
                writeln!(f, "  {} {location}", idx.bold())?;
                writeln!(f, "      {}", format!("\"{}\"", source.excerpt).dimmed())?;
            }
        }

        Ok(())
    }
}
```

A RAG answer without sources is like a parrot that speaks but won't say where it learned. Now the explorer can verify the answer against the original treasure.

## The Parrot's Muttering

Our parrot (DeepSeek-R1) is a *reasoning* model. Before it speaks wisdom, it thinks out loud:

```
<think>
The user asks about ownership in Rust. Let me look at the context...
Based on the chunks provided, I can see...
</think>
Ownership in Rust ensures memory safety without a garbage collector.
```

Those `<think>` blocks are fascinating for understanding how the parrot reasons, but we don't want them in the final answer. Our feather-grooming function handles this:

```rust
/// Strip `<think>...</think>` tags (used by reasoning models like Deepseek-r1).
pub fn strip_think_tags(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;

    while let Some((before, after_open)) = remaining.split_once("<think>") {
        result.push_str(before);
        match after_open.split_once("</think>") {
            Some((_, after_close)) => remaining = after_close,
            // Unclosed tag — the parrot trailed off mid-thought
            None => return result.trim().to_owned(),
        }
    }
    result.push_str(remaining);

    result.trim().to_owned()
}
```

This function is `pub` — if you later add a `summarize` module that also uses a reasoning model, you import it instead of copy-pasting. Duplicated logic is the reef that scrapes you when you fix a bug in one copy but forget the other.

> **Expedition tip**: If you switch to a non-reasoning parrot (like `llama3.2` or `mistral`), there won't be any `<think>` tags. The function handles that gracefully — no tags, no stripping, text passes through unchanged.

## Reef Warning: UTF-8 and Text Truncation

The `truncate_to_excerpt` function for source previews needs care. If you naively write `&text[..150]`, you'll panic on multi-byte characters — `"café"[..3]` slices into the middle of `é` and Rust rightfully refuses. Use `char_indices()` to find a safe boundary:

```rust
fn truncate_to_excerpt(text: &str, max_len: usize) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.len() <= max_len {
        return trimmed;
    }
    let boundary = trimmed
        .char_indices()
        .take_while(|&(i, _)| i < max_len)
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    let boundary = trimmed[..boundary].rfind(' ').unwrap_or(boundary);
    format!("{}...", &trimmed[..boundary])
}
```

The `rfind(' ')` prefers breaking at a word boundary — nicer excerpts for the explorer.

## The Parrot's Perch

Now the interactive chat — you ask, the crabs dive, the parrot answers:

```rust
// src/bin/chat.rs
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use owo_colors::OwoColorize;
use mini_rag::{DB_PATH, db, embed, rag};
```

RAG queries take a few seconds — the crabs need to dive and the parrot needs to reason. A blank terminal during that time feels broken. A braille spinner gives the explorer something to watch:

```rust
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn start_spinner(message: &str) -> mpsc::Sender<()> {
    let (tx, rx) = mpsc::channel();
    let msg = message.to_owned();
    std::thread::spawn(move || {
        let mut i = 0;
        #[allow(clippy::indexing_slicing)]
        loop {
            print!(
                "\r\x1b[2K{} {msg}",
                SPINNER_FRAMES[i % SPINNER_FRAMES.len()].yellow()
            );
            std::io::stdout().flush().ok();
            if rx.recv_timeout(Duration::from_millis(80)).is_ok() {
                break;
            }
            i += 1;
        }
        print!("\r\x1b[2K");
        std::io::stdout().flush().ok();
    });
    tx
}
```

The spinner runs on a separate OS thread (not a tokio task) so it keeps animating even when the async runtime is busy waiting for the LLM response.

> **Expedition tip**: Our hand-rolled spinner is ~20 lines. For a more polished expedition, consider [indicatif](https://crates.io/crates/indicatif) (progress bars, spinners, multi-bar support) or [spinners](https://crates.io/crates/spinners) (80+ spinner styles, minimal API). We kept ours dependency-free since it's the only place we need it.

And the main loop:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let conn = db::init_db(Path::new(DB_PATH)).await?;
    let client = mini_rag::ollama_client()?;
    let embedding_model = embed::embedding_model(&client);

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    println!(
        "{} — ask questions about your ingested documents",
        "rag-chat".bold().cyan()
    );
    println!("Type {} or {} to leave.\n", "exit".dimmed(), "quit".dimmed());

    loop {
        print!("{} ", ">".bold().green());
        stdout.flush()?;

        let mut line = String::new();
        let bytes_read = stdin.lock().read_line(&mut line)?;
        if bytes_read == 0 {
            break; // EOF — the explorer has left the island
        }

        let question = line.trim();
        if question.is_empty()
            || question == "exit"
            || question == "quit"
        {
            break;
        }

        let spinner = start_spinner("Searching and thinking...");
        let result = rag::query(
            &client, &embedding_model, &conn, question,
        ).await;
        spinner.send(()).ok();
        std::thread::sleep(Duration::from_millis(10));

        match result {
            Ok(response) => println!("\n{response}"),
            Err(e) => eprintln!("\n{} {e}\n", "Error:".red().bold()),
        }
    }

    Ok(())
}
```

Notice how `println!("\n{response}")` calls the `Display` impl we wrote on `RagResponse` — the answer prints first, followed by the colored source list. The `owo-colors` crate handles terminal colors with zero allocation — `.bold()`, `.green()`, `.dimmed()` are all zero-cost method chains.

> **Going further — better CLI**: For a permanent settlement, consider [rustyline](https://crates.io/crates/rustyline) for readline support (history, tab completion at the perch), [ratatui](https://crates.io/crates/ratatui) for a full TUI, or [clap](https://crates.io/crates/clap) for proper argument parsing (`--db-path`, `--chunk-size`). Our simple stdin loop works fine for the expedition.

## Talking to the Rust Books

```bash
$ cargo run --bin rag-chat

rag-chat — ask questions about your ingested documents
Type exit or quit to leave.

> What is the newtype pattern in Rust?
⠹ Searching and thinking...

Ahoy there, human! 🦜 Let's talk about the **newtype pattern**! *squawk!*

The newtype pattern wraps an existing type in a single-field tuple struct:
  pub struct UserId(u64);
This gives it a distinct type identity — you can't accidentally mix a
UserId with a raw u64. BRAWWK!

Why use it? 🦀
- Enforces invariants: no accidental type mixing
- Hides implementation: users don't see the wrapped type
- Orphan rule workaround: implement foreign traits on foreign types

Rusties love this pattern for building robust systems. 🏝️ *ruffles feathers*

─── Sources ───
  [1] comprehensive-rust (comprehensive-rust.pdf)
      "The newtype pattern wraps an existing type in a single-field tuple..."
  [2] rust-design-patterns (rust-design-patterns.pdf)
      "Newtype is a pattern that provides strong type distinctions..."

> exit
```

The parrot answers with flair — and the sources let the explorer verify against the original scrolls. Note that the parrot's response contains markdown formatting (headers, code blocks, bold). In a production system, you'd render this nicely with a crate like [termimad](https://crates.io/crates/termimad) or [termcolor-markdown](https://crates.io/crates/termcolor-markdown). Our humble `println!` does the job for now.

We're chatting with the Comprehensive Rust book and Rust Design Patterns — locally, on our island, with no cloud parrots, with answers grounded in the actual treasure contents, and with source attribution so the explorer can verify.

The crabs did their job beautifully.

## The Expedition Manifest

Let's count what we packed:

| Module | Lines | Island Role |
|--------|-------|-------------|
| `extract.rs` | 8 | The bottle opener |
| `chunk.rs` | 58 | The coconut cracker (+ 72 lines of tests) |
| `embed.rs` | 30 | The Pearl Forge |
| `db.rs` | 150 | The Smart Beach |
| `rag.rs` | 128 | The parrot whisperer (+ 47 lines of tests) |
| `lib.rs` | 15 | The base camp |
| `bin/sync.rs` | 142 | The gathering outpost |
| `bin/chat.rs` | 84 | The parrot's perch |
| **Total** | **~734** | **(~615 without tests)** |

About 615 lines of actual code. A fully functional, snake-free, local-first RAG system built entirely on Crab Island.

## Future Expeditions

This island has more to explore. Some trails for the adventurous:

**Better coconut cracking:**
- Sentence-aware splitting (crack on `.` boundaries, group to size)
- Heading-aware splitting (if your scrolls have markdown structure)
- [text-splitter](https://crates.io/crates/text-splitter) for semantic chunking with tiktoken

**Smarter diving:**
- Hybrid search: combine pearl diving (vectors) with word matching (FTS5) on the same beach
- Re-ranking: use a second model to re-score the top results
- Metadata filtering: dive only in certain sections of the beach

**Island infrastructure:**
- [Turso](https://turso.tech/) cloud sync: replicate your beach to the cloud (backup!)
- Swap parrots: point rig-core at OpenAI or Anthropic — one line change
- Build an [MCP server](https://modelcontextprotocol.io/): let AI agents from other islands query your beach

**Creature comforts:**
- [rustyline](https://crates.io/crates/rustyline) for readline support (history, tab completion at the perch)
- [ratatui](https://crates.io/crates/ratatui) for a full TUI — the parrot deserves a palace
- Streaming responses (rig-core supports it — watch the parrot think in real time)

## Farewell from Crab Island

We started this expedition wondering: can you build a RAG system without snakes? The answer is a resounding yes — and the island is more pleasant than expected.

We got type-safe pipelines where the island's guardian catches wiring mistakes before we waste time. We got `anyhow` keeping our error handling lean and context-rich. We got a single-file Smart Beach with both relational queries and vector search. And we got it all in ~615 lines of code, running entirely on our own island.

The Python continent across the sea has more established trade routes. You won't find a LangChain bazaar here with 500 integrations. But what you *will* find on Crab Island are well-crafted tools — rig-core, libsql, kreuzberg — that compose beautifully and let you build exactly what you need.

The crabs are happy. The parrot is wise. And there's not a snake in sight.

The full expedition code lives in the `mini-rag` directory. Clone the island, pull your parrot models (`ollama pull nomic-embed-text && ollama pull deepseek-r1`), sync some treasures, and start asking questions.

Happy treasure hunting!
