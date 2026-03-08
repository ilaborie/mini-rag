+++
title = "🥥  Part 3: Cracking Coconuts — Chunking Text"
description = "Splitting documents into overlapping chunks for embedding"
date = 2026-03-10
[taxonomies]
tags = ["rust", "rag"]
[extra]
illustration = "img/part3-coconuts.jpg"
photo_credit = "Corentin Largeron"
photo_credit_url = "https://unsplash.com/@croccol"
+++

*The Crab Island RAG Expedition — Part 3 of 6*

---

The crabs have gathered on the beach, claws clicking in anticipation. Before them lies the extracted text — 348,000 characters from the Comprehensive Rust scroll, 85,000 from Rust Design Patterns. That's a *lot* of text.

Now, a crab can't carry an entire coconut. It needs to crack it into pieces — small enough to handle, but large enough to still be nutritious. Too small, and you get dust. Too large, and nobody can carry it. And here's the trick: you want the pieces to *overlap* slightly, so that any juicy bit sitting right on the crack line appears in both pieces.

This is chunking. And it's quietly the most important part of the entire expedition.

## Why Cracking Matters So Much

Pop quiz: what's the single biggest factor in RAG answer quality?

Not the parrot's vocabulary. Not the embedding model. Not the database.

It's how you crack your coconuts.

Feed the parrot an entire 10-page chapter and it'll give you a vague, meandering answer. Feed it the *exact right paragraph* and it'll nail it. Chunking is the art of breaking text into pieces that are small enough to be precise, but large enough to carry meaning.

## The Sliding Window: A Crab's Best Friend

There are many cracking strategies:

| Strategy | Idea | Complexity |
|----------|------|------------|
| Fixed-size | Crack every N characters | Simple |
| Sentence-based | Crack on sentence boundaries | Medium |
| Semantic | Crack on topic changes | Complex (needs a model!) |
| Recursive | Try big cracks, fall back to smaller | Medium |

We're going with **fixed-size with overlap** — the simplest technique that actually works well. The overlap is the secret sauce: it ensures that any important idea sitting right on a crack line appears in both pieces.

Picture a crab walking along the coconut with a measuring shell:

```
Text:  [=======piece 1========]
                          [=======piece 2========]
                                             [=======piece 3========]
                          ←---→
                           overlap
```

Each piece is 500 characters. Each new piece starts 450 characters after the previous one (500 - 50 overlap). A sentence that gets cracked at position 490? It still appears fully in the next piece.

## The Code

```rust
// src/chunk.rs
#[derive(Debug)]
pub struct Chunk {
    pub index: usize,
    pub content: String,
}

#[must_use]
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<Chunk> {
    let chars: Vec<char> = text.chars().collect();
    let overlap = overlap.min(chunk_size / 2);
    let step = chunk_size - overlap;

    if chars.len() <= chunk_size {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }
        return vec![Chunk {
            index: 0,
            content: trimmed.to_owned(),
        }];
    }

    let mut chunks: Vec<_> = chars
        .windows(chunk_size)
        .step_by(step)
        .enumerate()
        .filter_map(|(i, window)| {
            let content: String = window.iter().collect();
            let trimmed = content.trim();
            (!trimmed.is_empty()).then(|| Chunk {
                index: i,
                content: trimmed.to_owned(),
            })
        })
        .collect();

    // Handle the last chunk if it wasn't covered by windows
    let last_start = chunks.len() * step;
    if last_start < chars.len() {
        let content: String = chars.get(last_start..).unwrap_or_default().iter().collect();
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            chunks.push(Chunk {
                index: chunks.len(),
                content: trimmed.to_owned(),
            });
        }
    }

    // Re-index after filtering
    for (i, chunk) in chunks.iter_mut().enumerate() {
        chunk.index = i;
    }

    chunks
}
```

Let's examine the crab's technique:

**Why clamp the overlap?** If someone passes an overlap larger than the chunk size, the subtraction `chunk_size - overlap` would underflow and panic. Even an overlap close to the chunk size makes no sense — you'd be re-reading almost the entire piece each time. Clamping to `chunk_size / 2` is a sensible guard: overlapping more than half a piece means you're doing more repeating than progressing.

**Why `windows()` + `step_by()`?** Rust's slice method `windows(n)` gives us a sliding window of size `n`, advancing one element at a time. Combined with `step_by(chunk_size - overlap)`, we get exactly the overlapping pieces we want — no manual index arithmetic needed. The standard library does the heavy lifting.

**Why `chars()` instead of bytes?** Because the island speaks many languages. If we split on bytes, we might slice a multi-byte character in half. Imagine cracking a coconut marked "Größe" (German for "size") right through the "ö" — we'd get invalid UTF-8. The crabs are more careful than that.

**Why handle the last chunk separately?** `windows()` only produces full-size windows. If the text doesn't divide evenly, the trailing piece (shorter than `chunk_size`) needs special handling.

**Why skip empty pieces?** Some coconuts have hollow sections — long stretches of whitespace from page breaks or formatting artifacts. No point carrying an empty shell. The crabs just toss those aside.

**Why `#[must_use]`?** If a crab cracks a coconut and nobody picks up the pieces, something has gone wrong. The island's guardian (the compiler) will raise an eyebrow.

## The Crabs Get to Work

Let's watch them crack our Rust books:

```rust
// Comprehensive Rust: ~348K characters
let text = extract::extract_text(
    Path::new("comprehensive-rust.pdf"),
).await?;
let chunks = chunk::chunk_text(&text, 500, 50);
// => 758 pieces!

// Rust Design Patterns: ~85K characters
let text = extract::extract_text(
    Path::new("rust-design-patterns.pdf"),
).await?;
let chunks = chunk::chunk_text(&text, 500, 50);
// => 187 pieces
```

758 coconut pieces from a ~350-page course. Each piece is a paragraph-sized morsel of Rust knowledge — small enough to carry, meaningful enough to be useful.

## Choosing the Right Crack Size

The magic numbers — 500 chars, 50 overlap — aren't carved in sacred coral. Here's how to think about them:

**Too small (e.g., 100 chars)**:
- Precise — you'll find exactly the right sentence
- But context-free — "It's unsafe" means nothing without knowing what "it" refers to
- More pieces = more work for the crabs, more pearls to bury

**Too large (e.g., 2000 chars)**:
- Plenty of context in each piece
- But diluted relevance — a piece about 5 different topics is vaguely relevant to all and strongly relevant to none
- Embedding models have limits too (nomic-embed-text handles ~8192 tokens)

**The overlap (50 chars)**:
- Too small: ideas at boundaries get lost in the crack
- Too large: you're carrying near-duplicate pieces, wasting energy
- 10% of chunk_size is a solid starting point

> **Expedition tip**: These numbers are worth tuning for your specific treasure. Dense academic papers might want smaller pieces. Conversational text might want larger ones. Start with 500/50, test your answer quality, adjust.

## Testing the Crab's Work

Here's something the crabs insist on — and you won't see in most treasure-hunting tutorials: **actual tests**.

```rust
#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_produces_no_chunks() {
        let chunks = chunk_text("", 500, 50);
        assert!(chunks.is_empty());
    }

    #[test]
    fn short_text_produces_single_chunk() {
        let chunks = chunk_text("hello world", 500, 50);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "hello world");
        assert_eq!(chunks[0].index, 0);
    }

    #[test]
    fn text_is_split_with_overlap() {
        let text = "a".repeat(1000);
        let chunks = chunk_text(&text, 500, 50);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content.len(), 500);
        assert_eq!(chunks[1].content.len(), 500);
        // Last chunk: 1000 - 900 = 100 chars
        assert_eq!(chunks[2].content.len(), 100);
    }

    #[test]
    fn chunks_have_sequential_indices() {
        let text = "x".repeat(2000);
        let chunks = chunk_text(&text, 500, 50);
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }

    #[test]
    fn overlap_clamped_to_half_chunk_size() {
        let text = "a".repeat(1000);
        // overlap 999 > chunk_size/2 (250), gets clamped to 250
        let chunks = chunk_text(&text, 500, 999);
        let expected = chunk_text(&text, 500, 250);
        assert_eq!(chunks.len(), expected.len());
        for (a, b) in chunks.iter().zip(expected.iter()) {
            assert_eq!(a.content, b.content);
        }
    }

    #[test]
    fn emoji_not_split_across_chunks() {
        // 🦀 is a single char (4 bytes), place it at the chunk boundary
        let text = format!("{}🦀{}", "a".repeat(4), "b".repeat(5));
        // 10 chars total, chunk_size=5, overlap=1, step=4
        let chunks = chunk_text(&text, 5, 1);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "aaaa🦀");
        assert_eq!(chunks[1].content, "🦀bbbb");
        assert_eq!(chunks[2].content, "bb");
    }

    #[test]
    fn whitespace_only_chunks_are_skipped() {
        let text = format!("{}{}content", "a".repeat(500), " ".repeat(500));
        let chunks = chunk_text(&text, 500, 0);
        // First chunk: 500 'a's, second chunk: 500 spaces (skipped), third chunk: "content"
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].content, "content");
    }
}
```

Empty coconuts, tiny coconuts, the overlap math, overlap clamping, emoji safety, sequential numbering, hollow sections — all verified. The emoji test is a nice illustration: the 🦀 crab (4 bytes, but 1 char) sits right at the boundary and appears in both adjacent chunks thanks to the overlap — never sliced in half. Pure functions are a joy to test: no mocks, no async, no setup. Just `crack → check`.

```bash
cargo nextest run
# 7 tests run: 7 passed
```

The crabs are meticulous workers.

## Fancier Cracking Tools

Our sliding-window approach is the machete of chunking — simple, reliable, gets the job done. When you're ready for finer tools:

- **Sentence-aware cracking**: Split on `.` boundaries, then group sentences up to your size limit. Better coherence per piece.
- **[text-splitter](https://crates.io/crates/text-splitter)**: A Rust crate for semantic-aware text splitting with tiktoken integration. The power tool version.
- **Recursive cracking**: Try paragraphs first, then sentences, then characters. What LangChain's `RecursiveCharacterTextSplitter` does on the Python continent.
- **Heading-aware cracking**: If your text has structure (markdown headers), split on those boundaries. Each piece gets a natural topic.

> **Note — kreuzberg can crack coconuts too**: kreuzberg has a built-in `chunking` feature that splits text during extraction. So why did we write our own? Two reasons. First, rolling your own chunker teaches you what's happening — it's ~60 lines of pure Rust with no magic. Second, separating extraction from chunking gives you flexibility: you can experiment with chunk sizes, test with unit tests, or swap in a smarter strategy later without touching extraction. In production, you might prefer kreuzberg's built-in chunking for convenience.

But our 50-line cracker will carry you surprisingly far. The Comprehensive Rust book chunks up nicely, and our RAG system returns relevant answers. Upgrade the tool when the simple one stops working, not before.

## Beach Report

The crabs have cracked all the coconuts:

- **~60 lines** of pure, synchronous Rust
- **Zero dependencies** — just `String` and `Vec`
- **Unicode-safe** — handles multi-byte characters without cracking through the middle
- **Tested** — 7 tests covering every edge case
- **Configurable** — piece size and overlap are parameters, not constants

The beach is covered in neatly numbered coconut pieces. But here's the thing about Crab Island — it's not an ordinary island. The coconuts here don't contain milk. Crack one open, and inside you'll find a pearl — a shimmering, dense little sphere that somehow captures the *essence* of the text it held. The crabs have known this for generations: every piece of knowledge, once properly cracked, reveals a pearl.

Now it's time for the truly magical step — forging those pearls. Time to visit the pearl forge.
