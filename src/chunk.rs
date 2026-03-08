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
        // overlap 999 > chunk_size/2 (250), gets clamped to 250, step = 250
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
