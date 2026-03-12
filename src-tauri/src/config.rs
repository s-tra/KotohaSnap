use std::path::PathBuf;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 型定義
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub provider: String,
    /// プロバイダごとのモデル設定（空文字 = プロバイダのデフォルト）
    #[serde(default)]
    pub models: ProviderModels,
    #[serde(default)]
    pub api_keys: ApiKeys,
    #[serde(default)]
    pub custom_provider: CustomProvider,
    #[serde(default)]
    pub osc: OscConfig,
    #[serde(default)]
    pub osc_enabled: bool,
    #[serde(default = "default_true")]
    pub osc_prefix_enabled: bool,
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(default = "default_font_size")]
    pub font_size: u8,
    #[serde(default = "default_watch_dir")]
    pub watch_dir: PathBuf,
    #[serde(default = "default_prompt")]
    pub translation_prompt: String,
}

/// プロバイダごとのモデル設定
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderModels {
    #[serde(default)]
    pub anthropic: String,
    #[serde(default)]
    pub openai: String,
    #[serde(default)]
    pub groq: String,
    #[serde(default)]
    pub google: String,
    #[serde(default)]
    pub custom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ApiKeys {
    #[serde(default)]
    pub anthropic: String,
    #[serde(default)]
    pub openai: String,
    #[serde(default)]
    pub groq: String,
    #[serde(default)]
    pub google: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OscConfig {
    pub host: String,
    pub port: u16,
    pub address: String,
    /// チャンク分割送信時の間隔（秒）。デフォルト 4 秒。
    #[serde(default = "default_chunk_interval")]
    pub chunk_interval_secs: u64,
}

/// カスタム（OpenAI 互換）プロバイダの設定
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CustomProvider {
    #[serde(default)]
    pub display_name: String,
    /// チャット補完エンドポイント URL（例: http://localhost:1234/v1/chat/completions）
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    /// モデル一覧取得 URL（任意。例: http://localhost:1234/v1/models）
    #[serde(default)]
    pub models_url: String,
}

// ---------------------------------------------------------------------------
// Default 実装
// ---------------------------------------------------------------------------

impl Default for OscConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 9000,
            address: "/chatbox/input".to_string(),
            chunk_interval_secs: default_chunk_interval(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            models: ProviderModels::default(),
            api_keys: ApiKeys::default(),
            custom_provider: CustomProvider::default(),
            osc: OscConfig::default(),
            osc_enabled: false,
            osc_prefix_enabled: true,
            sound_enabled: true,
            is_enabled: false,
            font_size: default_font_size(),
            watch_dir: default_watch_dir(),
            translation_prompt: default_prompt(),
        }
    }
}

// ---------------------------------------------------------------------------
// ヘルパー
// ---------------------------------------------------------------------------

fn default_provider() -> String { "google".to_string() }
fn default_true() -> bool { true }
fn default_chunk_interval() -> u64 { 4 }
fn default_font_size() -> u8 { 13 }

fn config_file_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("OS の設定ディレクトリが取得できませんでした")?
        .join("vrc-translator");
    Ok(dir.join("config.toml"))
}

fn default_watch_dir() -> PathBuf {
    dirs::picture_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("VRChat")
}

fn default_prompt() -> String {
    "この画像に含まれるテキストを日本語に翻訳してください。翻訳結果のみを返してください。説明や補足は不要です。周辺部に映りこんだ断片的な文字列や不鮮明なものは無視してください。テキストが存在しない場合は「テキストなし」とだけ返してください。".to_string()
}

// ---------------------------------------------------------------------------
// 読み書き
// ---------------------------------------------------------------------------

pub fn load_config() -> Result<Config> {
    let path = config_file_path()?;

    if !path.exists() {
        tracing::info!("設定ファイルが見つかりません。デフォルト設定を使用します: {}", path.display());
        return Ok(Config::default());
    }

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("設定ファイルの読み込みに失敗しました: {}", path.display()))?;

    let config: Config = toml::from_str(&raw)
        .with_context(|| format!("設定ファイルのパースに失敗しました: {}", path.display()))?;

    tracing::info!("設定を読み込みました: {}", path.display());
    Ok(config)
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_file_path()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("設定ディレクトリの作成に失敗しました: {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(config)
        .context("設定のシリアライズに失敗しました")?;

    std::fs::write(&path, raw)
        .with_context(|| format!("設定ファイルの書き込みに失敗しました: {}", path.display()))?;

    tracing::info!("設定を保存しました: {}", path.display());
    Ok(())
}
