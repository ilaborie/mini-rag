pub mod chunk;
pub mod db;
pub mod embed;
pub mod extract;
pub mod rag;

use anyhow::Context;
use rig::providers::ollama;

pub const DB_PATH: &str = "mini_rag.db";
pub const DEEPSEEK_R1: &str = "deepseek-r1";

pub fn ollama_client() -> anyhow::Result<ollama::Client> {
    ollama::Client::new(rig::client::Nothing).context("failed to create Ollama client")
}
