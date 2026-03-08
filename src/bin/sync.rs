#![allow(clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Context;
use rig::providers::ollama;
use tracing_subscriber::EnvFilter;

use mini_rag::chunk;
use mini_rag::{DB_PATH, db, embed, extract};

const CHUNK_SIZE: usize = 500;
const CHUNK_OVERLAP: usize = 50;

fn file_mtime(path: &Path) -> anyhow::Result<i64> {
    let metadata = std::fs::metadata(path)?;
    let mtime = metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system time before UNIX epoch")?;
    #[allow(clippy::cast_possible_wrap)]
    Ok(mtime.as_secs() as i64)
}

fn collect_files(args: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for arg in args {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            walk_dir(&path, &mut files)?;
        } else if path.is_file() {
            files.push(path);
        } else {
            tracing::warn!("Skipping {}: not a file or directory", path.display());
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

fn title_from_path(path: &Path) -> String {
    path.file_stem().map_or_else(
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

    if let Some((existing_id, existing_mtime)) =
        db::find_document_by_path(conn, &source_path).await?
    {
        if existing_mtime == mtime {
            tracing::info!("Skipping {} (unchanged)", path.display());
            return Ok(());
        }
        tracing::info!("Re-ingesting {} (mtime changed)", path.display());
        db::delete_document(conn, existing_id).await?;
    }

    tracing::info!("Ingesting {}...", path.display());

    let content = extract::extract_text(path).await?;
    tracing::info!(
        "Extracted {} characters from {}",
        content.len(),
        path.display()
    );

    let title = title_from_path(path);
    let doc_id = db::insert_document(conn, &title, &source_path, mtime).await?;

    let chunks = chunk::chunk_text(&content, CHUNK_SIZE, CHUNK_OVERLAP);
    tracing::info!("Created {} chunks for {}", chunks.len(), path.display());

    for (i, c) in chunks.iter().enumerate() {
        let embedding = embed::embed_text(embedding_model, &c.content).await?;
        db::insert_chunk(conn, doc_id, i, &c.content, &embedding).await?;
        if i % 10 == 0 {
            tracing::info!("Embedded chunk {}/{}", i + 1, chunks.len());
        }
    }

    tracing::info!("Done ingesting {}", path.display());
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("info".parse().expect("valid directive")),
        )
        .init();

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
        if let Err(e) = ingest_file(&conn, &embedding_model, file).await {
            tracing::error!("Failed to ingest {}: {e}", file.display());
        }
    }

    Ok(())
}
