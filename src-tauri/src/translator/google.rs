use std::path::Path;
use std::time::Duration;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use super::Translator;
use crate::image_utils;

const REQUEST_TIMEOUT_SECS: u64 = 120;

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
pub const DEFAULT_MODEL: &str = "gemini-flash-latest";

// ---------------------------------------------------------------------------
// リクエスト / レスポンス型
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Request {
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Part {
    InlineData { inline_data: InlineData },
    Text { text: String },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Deserialize)]
struct Response {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: CandidateContent,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: Option<String>,
}

// ---------------------------------------------------------------------------
// GoogleTranslator
// ---------------------------------------------------------------------------

pub struct GoogleTranslator {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl GoogleTranslator {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: if model.is_empty() { DEFAULT_MODEL.to_string() } else { model.to_string() },
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .build()
                .expect("reqwest::Client の構築に失敗しました"),
        }
    }
}

#[async_trait]
impl Translator for GoogleTranslator {
    async fn translate(&self, image_path: &Path, prompt: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow!("Google の API キーが設定されていません。設定画面で入力してください。"));
        }

        let (image_bytes, mime_type) = image_utils::load_and_prepare(image_path).await?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);

        let url = format!("{}/{}:generateContent?key={}", API_BASE, self.model, self.api_key);

        let body = Request {
            contents: vec![Content {
                parts: vec![
                    Part::InlineData {
                        inline_data: InlineData {
                            mime_type: mime_type.to_string(),
                            data: b64,
                        },
                    },
                    Part::Text { text: prompt.to_string() },
                ],
            }],
        };

        let resp = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Google Gemini API へのリクエストに失敗しました")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Google Gemini API エラー (HTTP {}): {}", status, error_body));
        }

        let parsed: Response = resp.json().await
            .context("Google Gemini API レスポンスのパースに失敗しました")?;

        parsed.candidates.into_iter().next()
            .and_then(|c| c.content.parts.into_iter().find_map(|p| p.text))
            .ok_or_else(|| anyhow!("Google Gemini API レスポンスにテキストが含まれていませんでした"))
    }

    fn name(&self) -> &str { "google" }
    fn model_name(&self) -> &str { &self.model }
}
