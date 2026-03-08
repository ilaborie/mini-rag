#[derive(Debug)]
pub struct Chunk {
    pub index: usize,
    pub content: String,
}

#[must_use]
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<Chunk> {
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        let content: String = chars.get(start..end).unwrap_or_default().iter().collect();
        let trimmed = content.trim();

        if !trimmed.is_empty() {
            chunks.push(Chunk {
                index: chunks.len(),
                content: trimmed.to_owned(),
            });
        }

        if end == chars.len() {
            break;
        }
        start += chunk_size - overlap;
    }

    chunks
}

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
    fn whitespace_only_chunks_are_skipped() {
        let text = format!("{}{}content", "a".repeat(500), " ".repeat(500));
        let chunks = chunk_text(&text, 500, 0);
        // First chunk: 500 'a's, second chunk: 500 spaces (skipped), third chunk: "content"
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].content, "content");
    }
}
