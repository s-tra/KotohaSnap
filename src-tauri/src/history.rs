use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 翻訳ログの1エントリ
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TranslationEntry {
    pub timestamp: DateTime<Utc>,
    pub image_path: PathBuf,
    pub translated_text: String,
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// 生成済みサムネイルのパス（None の場合は image_path をフォールバック表示）
    #[serde(default)]
    pub thumbnail_path: Option<PathBuf>,
}
