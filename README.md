# mini-rag

A minimal RAG (Retrieval-Augmented Generation) system in Rust. Built with rig-core, libsql, and kreuzberg.

## Prerequisites

- Rust 1.91.1+ (edition 2024)
- [Ollama](https://ollama.com/) running locally
- [pdfium](https://github.com/nicehash/nicehash-quickminer/releases) library installed for PDF extraction

Pull the required models:

```bash
ollama pull nomic-embed-text
ollama pull deepseek-r1
```

## Sample documents

Download the PDF files used in the blog series:

```bash
curl -LO https://google.github.io/comprehensive-rust/comprehensive-rust.pdf
curl -LO https://rust-unofficial.github.io/patterns/rust-design-patterns.pdf
```

## Usage

Ingest documents:

```bash
cargo run --bin rag-sync -- comprehensive-rust.pdf rust-design-patterns.pdf
```

Chat with your documents:

```bash
cargo run --bin rag-chat
```
