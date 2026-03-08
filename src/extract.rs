use std::path::Path;

use kreuzberg::ExtractionConfig;

pub async fn extract_text(path: &Path) -> anyhow::Result<String> {
    let result = kreuzberg::extract_file(path, None, &ExtractionConfig::default()).await?;
    Ok(result.content)
}
