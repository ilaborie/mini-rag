use std::fmt::{self, Write as _};

use libsql::Connection;
use owo_colors::OwoColorize;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::ollama;

use crate::DEEPSEEK_R1;
use crate::db;
use crate::embed;

const TOP_K: usize = 5;
const EXCERPT_LEN: usize = 150;

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

fn truncate_to_excerpt(text: &str, max_len: usize) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.len() <= max_len {
        return trimmed;
    }
    // Find a safe char boundary that includes the last character starting within max_len
    let boundary = trimmed
        .char_indices()
        .take_while(|&(i, _)| i < max_len)
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    let boundary = trimmed[..boundary].rfind(' ').unwrap_or(boundary);
    format!("{}...", &trimmed[..boundary])
}

/// Strip `<think>...</think>` tags (used by reasoning models like Deepseek-r1).
pub fn strip_think_tags(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;

    while let Some((before, after_open)) = remaining.split_once("<think>") {
        result.push_str(before);
        match after_open.split_once("</think>") {
            Some((_, after_close)) => remaining = after_close,
            None => return result.trim().to_owned(),
        }
    }
    result.push_str(remaining);

    result.trim().to_owned()
}

pub async fn query(
    client: &ollama::Client,
    embedding_model: &ollama::EmbeddingModel,
    conn: &Connection,
    question: &str,
) -> anyhow::Result<RagResponse> {
    tracing::info!("RAG query: {question}");

    // 1. Search for relevant chunks with source info
    let query_embedding = embed::embed_text(embedding_model, question).await?;
    let hits = db::vector_search(conn, &query_embedding, TOP_K).await?;

    // 2. Build context and collect sources in one pass
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

    // 4. Query LLM with explicit context
    let agent = client
        .agent(DEEPSEEK_R1)
        .preamble(
            "You are Kwaak 🦜, the wise parrot of Crab Island. \
             You speak with parrot flair — sprinkle in the occasional *squawk!*, \
             *BRAWWK!*, or *ruffles feathers*, and use emojis like 🦀🥥🏝️. \
             But under the plumage, you are precise and knowledgeable. \
             Use the provided context to answer accurately. \
             If the context doesn't contain enough information, squawk honestly — \
             never make up answers. Keep answers concise.",
        )
        .build();

    let prompt = format!("Context:\n{context}\n\nQuestion: {question}");
    let response = agent.prompt(prompt).await?;

    Ok(RagResponse {
        answer: strip_think_tags(&response),
        sources,
    })
}

impl fmt::Display for RagResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.answer)?;

        if !self.sources.is_empty() {
            writeln!(f, "\n{}", "─── Sources ───".dimmed())?;
            for (i, source) in self.sources.iter().enumerate() {
                let idx = format!("[{}]", i + 1);
                let location = format!("{} ({})", source.title.green(), source.path.dimmed());
                writeln!(f, "  {} {location}", idx.bold())?;
                writeln!(f, "      {}", format!("\"{}\"", source.excerpt).dimmed())?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_think_tags_removes_reasoning() {
        let input = "<think>internal reasoning</think>The actual answer.";
        assert_eq!(strip_think_tags(input), "The actual answer.");
    }

    #[test]
    fn strip_think_tags_handles_multiple() {
        let input = "<think>a</think>Hello <think>b</think>world";
        assert_eq!(strip_think_tags(input), "Hello world");
    }

    #[test]
    fn strip_think_tags_no_tags() {
        let input = "Just plain text";
        assert_eq!(strip_think_tags(input), "Just plain text");
    }

    #[test]
    fn strip_think_tags_unclosed() {
        let input = "Before<think>unclosed reasoning";
        assert_eq!(strip_think_tags(input), "Before");
    }

    #[test]
    fn truncate_to_excerpt_short_text() {
        assert_eq!(truncate_to_excerpt("hello world", 50), "hello world");
    }

    #[test]
    fn truncate_to_excerpt_long_text() {
        let result = truncate_to_excerpt("this is a longer text that should be truncated", 20);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 25);
    }

    #[test]
    fn truncate_to_excerpt_multibyte() {
        let text = "café résumé naïve";
        let result = truncate_to_excerpt(text, 6);
        assert!(result.ends_with("..."));
    }
}
