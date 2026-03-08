use rig::client::EmbeddingsClient;
use rig::embeddings::EmbeddingModel as _;
use rig::providers::ollama;

const NOMIC_EMBED_TEXT: &str = "nomic-embed-text";
pub const EMBEDDING_DIMS: usize = 768;

#[must_use]
pub fn embedding_model(client: &ollama::Client) -> ollama::EmbeddingModel {
    client.embedding_model_with_ndims(NOMIC_EMBED_TEXT, EMBEDDING_DIMS)
}

pub async fn embed_text(model: &ollama::EmbeddingModel, text: &str) -> anyhow::Result<Vec<f32>> {
    let embedding = model.embed_text(text).await?;
    Ok(to_f32(&embedding.vec))
}

pub async fn embed_texts(
    model: &ollama::EmbeddingModel,
    texts: impl IntoIterator<Item = String> + Send,
) -> anyhow::Result<Vec<Vec<f32>>> {
    let embeddings = model.embed_texts(texts).await?;
    Ok(embeddings.iter().map(|e| to_f32(&e.vec)).collect())
}

// rig returns Vec<f64>, libsql F32_BLOB needs f32
#[allow(clippy::cast_possible_truncation)]
fn to_f32(vec: &[f64]) -> Vec<f32> {
    vec.iter().map(|&v| v as f32).collect()
}
