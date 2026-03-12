use std::path::Path;
use std::time::Duration;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use super::Translator;
use crate::image_utils;

const REQUEST_TIMEOUT_SECS: u64 = 60;
const CONNECT_TIMEOUT_SECS: u64 = 10;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
pub const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";
const MAX_TOKENS: u32 = 1024;

// ---------------------------------------------------------------------------
// リクエスト / レスポンス型
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<Message<'a>>,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: Vec<ContentBlock<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock<'a> {
    Image { source: ImageSource<'a> },
    Text { text: &'a str },
}

#[derive(Serialize)]
struct ImageSource<'a> {
    #[serde(rename = "type")]
    source_type: &'a str,
    media_type: &'a str,
    data: String,
}

#[derive(Deserialize)]
struct Response {
    content: Vec<ResponseContent>,
}

#[derive(Deserialize)]
struct ResponseContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

// ---------------------------------------------------------------------------
// AnthropicTranslator
// ---------------------------------------------------------------------------

pub struct AnthropicTranslator {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicTranslator {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: if model.is_empty() { DEFAULT_MODEL.to_string() } else { model.to_string() },
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .build()
                .expect("reqwest::Client の構築に失敗しました"),
        }
    }
}

#[async_trait]
impl Translator for AnthropicTranslator {
    async fn translate(&self, image_path: &Path, prompt: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow!("Anthropic の API キーが設定されていません。設定画面で入力してください。"));
        }

        let (image_bytes, media_type) = image_utils::load_and_prepare(image_path).await?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);

        let body = Request {
            model: &self.model,
            max_tokens: MAX_TOKENS,
            messages: vec![Message {
                role: "user",
                content: vec![
                    ContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64",
                            media_type,
                            data: b64,
                        },
                    },
                    ContentBlock::Text { text: prompt },
                ],
            }],
        };

        let resp = self.client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Anthropic API へのリクエストに失敗しました")?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API エラー (HTTP {}): {}", status, error_body));
        }

        let parsed: Response = resp.json().await
            .context("Anthropic API レスポンスのパースに失敗しました")?;

        parsed.content.into_iter()
            .find(|c| c.kind == "text")
            .and_then(|c| c.text)
            .ok_or_else(|| anyhow!("Anthropic API レスポンスにテキストブロックが含まれていませんでした"))
    }

    fn name(&self) -> &str { "anthropic" }
    fn model_name(&self) -> &str { &self.model }
}
