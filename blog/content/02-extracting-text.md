+++
title = "🍾  Part 2: Opening the Bottles — Text Extraction"
description = "Extracting text from PDFs, EPUBs, and HTML with kreuzberg"
date = 2026-03-09
[taxonomies]
tags = ["rust", "rag", "kreuzberg"]
[extra]
illustration = "img/part2-bottles.jpg"
photo_credit = "Jayne Harris"
photo_credit_url = "https://unsplash.com/@jayneharr33"
+++

*The Crab Island RAG Expedition — Part 2 of 6*

---

Day two on Crab Island. The morning mist lifts and reveals the shoreline, littered with treasures from across the seven seas — sealed bottles with letters inside, scrolls wrapped in oilcloth, maps etched on strange flat stones. Some look ancient and weathered. Others are suspiciously well-formatted.

These are our documents. PDFs, EPUBs, DOCX files — each one a different container holding knowledge we want to search through. Before the crabs can do anything useful with this treasure, we need to *open* it and get the text out.

Here's the thing about these containers: they're all different. A bottle (PDF) isn't text at all — it's a set of page layout instructions. "Draw glyph 'H' at coordinates (72.4, 540.2). Draw glyph 'e' at (78.1, 540.2)." Turning that back into the word "Hello" is a feat of reverse engineering. A scroll (EPUB) is really a bundle of HTML pages wrapped in a zip file. A carved stone (DOCX) is XML wearing a trench coat.

Fortunately, we don't have to crack each one by hand. We have kreuzberg — the island's universal bottle opener.

## kreuzberg: One Tool for Every Container

[kreuzberg](https://crates.io/crates/kreuzberg) handles 75+ document formats through a single async function call. PDFs, EPUBs, DOCX, HTML, images (via OCR), plain text — hand it any container from the shore and it extracts what's inside.

Here's our entire extraction module:

```rust
// src/extract.rs
use std::path::Path;

use kreuzberg::ExtractionConfig;

pub async fn extract_text(path: &Path) -> anyhow::Result<String> {
    let result = kreuzberg::extract_file(path, None, &ExtractionConfig::default()).await?;
    Ok(result.content)
}
```

Four lines of actual code. The crabs hand kreuzberg a container, it hands back the text inside. No fuss, no format-specific logic.

`extract_file` takes a path, an optional MIME type hint (we pass `None` — "you figure it out"), and a config (we use defaults). The `?` works seamlessly because we use `anyhow::Result`, which automatically converts any error implementing `std::error::Error`.

## Cracking Open the Rust Books

Let's try it on some real treasures. Imagine we've found two legendary scrolls on the beach: [Comprehensive Rust](https://google.github.io/comprehensive-rust/comprehensive-rust.pdf) (Google's Rust course) and [Rust Design Patterns](https://rust-unofficial.github.io/patterns/rust-design-patterns.pdf):

```rust
let text = extract::extract_text(
    Path::new("comprehensive-rust.pdf"),
).await?;
tracing::info!("Extracted {} characters", text.len());
// => Extracted 348572 characters
```

That's 348,000 characters of Rust wisdom, liberated from its PDF prison. And the design patterns scroll:

```rust
let text = extract::extract_text(
    Path::new("rust-design-patterns.pdf"),
).await?;
tracing::info!("Extracted {} characters", text.len());
// => Extracted 85431 characters
```

Same function, different containers, same result: pure text. If tomorrow a crab washes ashore with an EPUB or a DOCX, the same code handles it. One bottle opener to rule them all.

## The Hidden Reef: pdfium

Every tropical island has a hidden reef, and here's ours. kreuzberg uses [pdfium](https://pdfium.googlesource.com/pdfium/) under the hood for PDF extraction — the same C library Chrome uses to render PDFs. It needs to be available at runtime.

If you try to open a PDF bottle and get:

```
kreuzberg error: pdfium library not found
```

...you've hit the reef. You need to download `libpdfium.dylib` (macOS) or `libpdfium.so` (Linux) from the [pdfium-binaries](https://github.com/nicehash/nicehash-quickminer/releases) releases and place it where the crabs can find it:

```bash
# macOS (Apple Silicon)
mkdir -p ~/lib && cp libpdfium.dylib ~/lib/
```

> **Expedition tip**: The kreuzberg crate has a `pdf` feature that must be explicitly enabled — the default features don't include PDF support! Make sure your manifest says `kreuzberg = { version = "4.4.4", features = ["pdf"] }`. There's also a `bundled-pdfium` feature, but despite its promising name, it doesn't actually bundle pdfium. It just enables the `pdf` feature. We all ran aground on that reef at least once.

## Why Plain Text?

A keen-eyed explorer might ask: "We're throwing away structure! Headlines, bold text, tables — all gone. Isn't that wasteful?"

Fair point! But consider:

1. **The crabs work with meaning, not formatting.** Embedding models convert *text* to vectors. They don't know what bold means.
2. **Coconut cracking is simpler.** Our chunker (next post) uses a sliding window — it doesn't need to know about headings or paragraphs.
3. **It's model-agnostic.** Whether our parrot speaks nomic-embed-text, OpenAI's ada, or something else entirely, they all eat plain text.

Could we do better? Absolutely — preserve markdown structure, split on heading boundaries, keep tables intact. But that's optimization for a future expedition. The crabs on this island believe in starting simple. The Rust Design Patterns scroll itself would approve — start with a working solution, refine later.

> **Going further — OCR: reading faded carvings**: Some bottles on the beach contain images instead of text — scanned documents, photos of whiteboards, hand-drawn diagrams. kreuzberg can read those too via OCR (Optical Character Recognition), powered by [Tesseract](https://github.com/tesseract-ocr/tesseract) or [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR). Pass an image file to the same `extract_file` function and it extracts whatever text it can find. You'll need to enable the `ocr` or `paddle-ocr` feature in `Cargo.toml` and install the OCR engine on your system (`brew install tesseract` on macOS for Tesseract). Results vary with image quality — a crisp scan yields great text, a blurry photo yields crab scratches. For the expedition we stick to digital documents, but if your treasure trove includes scanned PDFs or images, kreuzberg has you covered without any code changes.

## Other Bottle Openers

kreuzberg is our pick, but the island market has alternatives:

- **[pdf-extract](https://crates.io/crates/pdf-extract)**: Pure Rust, PDF-only. Lighter to carry, no pdfium reef to navigate. Perfect if you only find PDF bottles on your beach.
- **[lopdf](https://crates.io/crates/lopdf)**: Low-level PDF manipulation. For the explorer who wants to understand every byte of the container.
- **[docx-rs](https://crates.io/crates/docx-rs)**: DOCX-specific. When your shore is covered in carved stones and nothing else.

We chose kreuzberg because we expect all kinds of treasures to wash up — PDFs today, EPUBs and research papers tomorrow. One tool, one function, every format.

## Beach Report

Our extraction outpost is ready:

- **4 lines of code** (not counting imports)
- **Format-agnostic** — PDFs, EPUBs, DOCX, HTML, and 70+ more
- **Async** — won't block the crabs while they wait for a large PDF to process
- **Error-handled** — kreuzberg errors flow naturally through `anyhow` via `?`

The bottles are open and the messages are spread out on the beach. But these aren't ordinary messages — they're treasure maps, and they all point to the same thing: a grove of coconut palms deeper on the island. The text inside those coconuts holds the real knowledge. But a crab can't carry a whole coconut — it needs to crack them into manageable pieces first.
