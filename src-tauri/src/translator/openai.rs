use std::path::Path;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use super::Translator;
use crate::image_utils;

const API_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MODEL: &str = "gpt-4o";
const MAX_TOKENS: u32 = 1024;

// ---------------------------------------------------------------------------
// リクエスト型
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
    content: Vec<ContentPart>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    ImageUrl { image_url: ImageUrl },
    Text     { text: String },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Deserialize)]
struct Response {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

// ---------------------------------------------------------------------------
// OpenAITranslator
// ---------------------------------------------------------------------------

pub struct OpenAITranslator {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAITranslator {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: if model.is_empty() { DEFAULT_MODEL.to_string() } else { model.to_string() },
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Translator for OpenAITranslator {
    async fn translate(&self, image_path: &Path, prompt: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow!("OpenAI の API キーが設定されていません。設定画面で入力してください。"));
        }

        let (image_bytes, mime) = image_utils::load_and_prepare(image_path).await?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);
        let data_url = format!("data:{mime};base64,{b64}");

        let body = Request {
            model: &self.model,
            max_tokens: MAX_TOKENS,
            messages: vec![Message {
                role: "user",
                content: vec![
                    ContentPart::ImageUrl { image_url: ImageUrl { url: data_url } },
                    ContentPart::Text     { text: prompt.to_string() },
                ],
            }],
        };

        let resp = self.client
            .post(API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("OpenAI API へのリクエストに失敗しました")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API エラー (HTTP {}): {}", status, body));
        }

        let parsed: Response = resp.json().await
            .context("OpenAI API レスポンスのパースに失敗しました")?;

        parsed.choices.into_iter().next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| anyhow!("OpenAI API レスポンスにテキストが含まれていませんでした"))
    }

    fn name(&self) -> &str { "openai" }
    fn model_name(&self) -> &str { &self.model }
}
