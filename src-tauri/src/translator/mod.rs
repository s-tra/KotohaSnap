use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;

pub mod anthropic;
pub mod custom;
pub mod google;
pub mod groq;
pub mod openai;

// ---------------------------------------------------------------------------
// Translator trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate(&self, image_path: &Path, prompt: &str) -> anyhow::Result<String>;
    /// プロバイダ名
    fn name(&self) -> &str;
    /// 使用モデル名
    fn model_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// default_models
// ---------------------------------------------------------------------------

/// 各プロバイダのデフォルトモデル名を返す
pub fn default_models() -> std::collections::HashMap<&'static str, &'static str> {
    [
        ("anthropic", anthropic::DEFAULT_MODEL),
        ("openai",    openai::DEFAULT_MODEL),
        ("groq",      groq::DEFAULT_MODEL),
        ("google",    google::DEFAULT_MODEL),
    ]
    .into_iter()
    .collect()
}

// ---------------------------------------------------------------------------
// build_translator
// ---------------------------------------------------------------------------

/// 設定に応じた Translator を構築する
pub fn build_translator(config: &crate::config::Config) -> Arc<dyn Translator> {
    let models = &config.models;

    match config.provider.as_str() {
        "openai"  => Arc::new(openai::OpenAITranslator::new(&config.api_keys.openai, &models.openai)),
        "groq"    => Arc::new(groq::GroqTranslator::new(&config.api_keys.groq, &models.groq)),
        "google"  => Arc::new(google::GoogleTranslator::new(&config.api_keys.google, &models.google)),
        "custom" => {
            let cp = &config.custom_provider;
            Arc::new(custom::CustomTranslator::new(
                cp.api_url.clone(),
                cp.api_key.clone(),
                models.custom.clone(),
                cp.display_name.clone(),
            ))
        }
        _ => Arc::new(anthropic::AnthropicTranslator::new(&config.api_keys.anthropic, &models.anthropic)),
    }
}
