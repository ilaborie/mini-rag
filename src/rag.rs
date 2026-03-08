use std::fmt;

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

    while let Some(start) = remaining.find("<think>") {
        result.push_str(&remaining[..start]);
        if let Some(end) = remaining[start..].find("</think>") {
            remaining = &remaining[start + end + "</think>".len()..];
        } else {
            return result.trim().to_owned();
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

    // 2. Build context from search results
    let context: String = hits
        .iter()
        .enumerate()
        .map(|(i, hit)| format!("[{}] {}", i + 1, hit.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    // 3. Collect sources
    let sources: Vec<Source> = hits
        .iter()
        .map(|hit| Source {
            title: hit.doc_title.clone(),
            path: hit.doc_path.clone(),
            excerpt: truncate_to_excerpt(&hit.content, EXCERPT_LEN),
        })
        .collect();

    // 4. Query LLM with explicit context
    let agent = client
        .agent(DEEPSEEK_R1)
        .preamble(
            "You are a helpful assistant. Answer the user's question based on the provided context. \
             If the context doesn't contain enough information, say so. \
             Be concise and direct.",
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
