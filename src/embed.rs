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
    // rig returns Vec<f64>, libsql F32_BLOB needs f32
    #[allow(clippy::cast_possible_truncation)]
    let vec_f32 = embedding.vec.iter().map(|&v| v as f32).collect();
    Ok(vec_f32)
}
