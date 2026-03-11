use std::sync::atomic::Ordering;
use serde::Deserialize;
use tauri::{Emitter, Manager, State};

use crate::config::{self, Config};
use crate::history::TranslationEntry;
use crate::osc;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// 設定
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
pub fn save_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut new_config: Config,
) -> Result<(), String> {
    // トグル状態はメインウィンドウが管理するため、現在の AtomicBool の値を保持する
    new_config.is_enabled = state.is_enabled.load(Ordering::Relaxed);
    new_config.osc_enabled = state.osc_enabled.load(Ordering::Relaxed);
    config::save_config(&new_config).map_err(|e| e.to_string())?;
    state.apply_config(new_config);
    // 全ウィンドウに設定更新を通知
    let _ = app.emit("config_saved", ());
    Ok(())
}

// ---------------------------------------------------------------------------
// 有効/無効切り替え
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn set_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state.is_enabled.store(enabled, Ordering::Relaxed);
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.is_enabled = enabled;
        config::save_config(&cfg).map_err(|e| e.to_string())?;
    }
    tracing::info!("翻訳を {} にしました", if enabled { "ON" } else { "OFF" });
    Ok(())
}

#[tauri::command]
pub fn set_osc_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state.osc_enabled.store(enabled, Ordering::Relaxed);
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.osc_enabled = enabled;
        config::save_config(&cfg).map_err(|e| e.to_string())?;
    }
    tracing::info!("OSC 送信を {} にしました", if enabled { "ON" } else { "OFF" });
    Ok(())
}

#[tauri::command]
pub fn set_font_size(state: State<'_, AppState>, size: u8) -> Result<(), String> {
    let size = size.clamp(10, 20);
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.font_size = size;
    config::save_config(&cfg).map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 設定ウィンドウを開く
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub async fn open_about(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("about") {
        w.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "about",
        tauri::WebviewUrl::App("about.html".into()),
    )
    .title("about - vrc-translator")
    .inner_size(400.0, 300.0)
    .resizable(false)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        w.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("設定 - vrc-translator")
    .inner_size(580.0, 720.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// デフォルトモデル
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_default_models() -> std::collections::HashMap<&'static str, &'static str> {
    crate::translator::default_models()
}

// ---------------------------------------------------------------------------
// モデル一覧取得
// ---------------------------------------------------------------------------

/// 指定プロバイダからモデル一覧を取得する。
/// フォームの現在値をそのまま受け取る（未保存の API キーにも対応）。
#[tauri::command]
pub async fn fetch_models(
    provider: String,
    api_key: String,
    models_url: Option<String>,
) -> Result<Vec<String>, String> {
    match provider.as_str() {
        "anthropic" => fetch_anthropic_models(&api_key).await,
        "openai"    => fetch_openai_compat_models("https://api.openai.com/v1/models", &api_key).await,
        "groq"      => fetch_openai_compat_models("https://api.groq.com/openai/v1/models", &api_key).await,
        "google"    => fetch_google_models(&api_key).await,
        "custom"    => {
            let url = models_url
                .filter(|u| !u.is_empty())
                .ok_or_else(|| "カスタムプロバイダのモデル取得 URL が設定されていません".to_string())?;
            fetch_openai_compat_models(&url, &api_key).await
        }
        _ => Err(format!("不明なプロバイダ: {}", provider)),
    }
}

// ---------------------------------------------------------------------------
// 翻訳履歴
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Result<Vec<TranslationEntry>, String> {
    Ok(state.get_history())
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.clear_history();
    Ok(())
}

// ---------------------------------------------------------------------------
// ファイル / URL を OS のデフォルトアプリで開く
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn open_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(&path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(&url, None::<&str>).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// OSC テスト
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn test_osc(state: State<'_, AppState>) -> Result<(), String> {
    let osc_config = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.osc.clone()
    };
    osc::test_send(&osc_config).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// モデル取得ヘルパー
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
}

/// Anthropic モデル一覧（GET /v1/models）
async fn fetch_anthropic_models(api_key: &str) -> Result<Vec<String>, String> {
    if api_key.is_empty() {
        return Err("API キーが入力されていません".to_string());
    }
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic モデル取得エラー: {}", body));
    }

    let parsed: ModelsResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Ok(ids)
}

/// Google Gemini モデル一覧（GET /v1beta/models?key=...）
async fn fetch_google_models(api_key: &str) -> Result<Vec<String>, String> {
    if api_key.is_empty() {
        return Err("API キーが入力されていません".to_string());
    }

    #[derive(Deserialize)]
    struct GoogleModelsResponse {
        models: Vec<GoogleModelInfo>,
    }
    #[derive(Deserialize)]
    struct GoogleModelInfo {
        name: String,
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );
    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Google モデル取得エラー: {}", body));
    }

    let parsed: GoogleModelsResponse = resp.json().await.map_err(|e| e.to_string())?;
    // "models/gemini-xxx" -> "gemini-xxx"
    let mut ids: Vec<String> = parsed.models
        .into_iter()
        .map(|m| m.name.trim_start_matches("models/").to_string())
        .filter(|id| id.starts_with("gemini-"))
        .collect();
    ids.sort();
    Ok(ids)
}

/// OpenAI 互換モデル一覧（GET <url>）
async fn fetch_openai_compat_models(url: &str, api_key: &str) -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let mut builder = client.get(url);
    if !api_key.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = builder.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("モデル取得エラー: {}", body));
    }

    let parsed: ModelsResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Ok(ids)
}

