use std::path::Path;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use super::Translator;
use crate::image_utils;

const MAX_TOKENS: u32 = 1024;

// ---------------------------------------------------------------------------
// リクエスト型（OpenAI 互換）
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Request {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
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
// CustomTranslator
// ---------------------------------------------------------------------------

pub struct CustomTranslator {
    api_url: String,
    api_key: String,
    model: String,
    display_name: String,
    client: reqwest::Client,
}

impl CustomTranslator {
    pub fn new(api_url: String, api_key: String, model: String, display_name: String) -> Self {
        Self { api_url, api_key, model, display_name, client: reqwest::Client::new() }
    }
}

#[async_trait]
impl Translator for CustomTranslator {
    async fn translate(&self, image_path: &Path, prompt: &str) -> Result<String> {
        if self.api_url.is_empty() {
            return Err(anyhow!(
                "カスタムプロバイダの API URL が設定されていません。設定画面で入力してください。"
            ));
        }
        if self.model.is_empty() {
            return Err(anyhow!(
                "カスタムプロバイダのモデルが設定されていません。設定画面で入力してください。"
            ));
        }

        let (image_bytes, mime) = image_utils::load_and_prepare(image_path).await?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);
        let data_url = format!("data:{mime};base64,{b64}");

        let body = Request {
            model: self.model.clone(),
            max_tokens: MAX_TOKENS,
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![
                    ContentPart::ImageUrl { image_url: ImageUrl { url: data_url } },
                    ContentPart::Text     { text: prompt.to_string() },
                ],
            }],
        };

        let mut builder = self.client.post(&self.api_url)
            .header("Content-Type", "application/json");
        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = builder
            .json(&body)
            .send()
            .await
            .context("カスタム API へのリクエストに失敗しました")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("カスタム API エラー (HTTP {}): {}", status, body));
        }

        let parsed: Response = resp.json().await
            .context("カスタム API レスポンスのパースに失敗しました")?;

        parsed.choices.into_iter().next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| anyhow!("カスタム API レスポンスにテキストが含まれていませんでした"))
    }

    fn name(&self) -> &str {
        if self.display_name.is_empty() { "custom" } else { &self.display_name }
    }
    fn model_name(&self) -> &str { &self.model }
}
