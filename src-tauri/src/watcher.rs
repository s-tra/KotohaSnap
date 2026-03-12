use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::{ModifyKind, RenameMode};
use tauri::{AppHandle, Emitter, Manager};
use tauri::async_runtime;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::history::TranslationEntry;
use crate::osc;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// 内部型
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, Clone)]
struct OscChunkProgress {
    current: usize,
    total: usize,
}

// ---------------------------------------------------------------------------
// パブリック API
// ---------------------------------------------------------------------------

pub fn spawn_watcher(app_handle: AppHandle) {
    async_runtime::spawn(async move {
        // JS の listen() が登録されるまで少し待つ
        sleep(Duration::from_millis(1500)).await;

        let state = app_handle.state::<AppState>();
        loop {
            let restart = Arc::clone(&state.watcher_restart);
            tokio::select! {
                result = watch_loop(&app_handle, &state) => {
                    if let Err(e) = result {
                        tracing::error!("watcher エラー: {e}");
                        let _ = app_handle.emit("watcher_error", e.to_string());
                        sleep(Duration::from_secs(3)).await;
                    }
                }
                _ = restart.notified() => {
                    tracing::info!("watcher を再起動します（設定変更）");
                    sleep(Duration::from_millis(200)).await;
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// 監視ループ
// ---------------------------------------------------------------------------

async fn watch_loop(app_handle: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    let watch_dir = {
        let config = state.config.lock().expect("config lock poisoned");
        config.watch_dir.clone()
    };

    if !watch_dir.exists() {
        let msg = format!("スクリーンショットフォルダが存在しません: {}", watch_dir.display());
        tracing::warn!("{msg}");
        let _ = app_handle.emit("watcher_error", &msg);
        sleep(Duration::from_secs(10)).await;
        return Ok(());
    }

    tracing::info!("監視開始（再帰）: {}", watch_dir.display());
    let _ = app_handle.emit(
        "watcher_status",
        format!("監視中: {}", watch_dir.display()),
    );

    let (tx, mut rx) = mpsc::unbounded_channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(
        move |res| { let _ = tx.send(res); },
        Config::default(),
    )?;

    watcher.watch(&watch_dir, RecursiveMode::Recursive)?;

    // path -> 最終検出時刻（5秒以内の重複イベントを除去）
    let mut recently_processed: HashMap<PathBuf, Instant> = HashMap::new();

    while let Some(res) = rx.recv().await {
        recently_processed.retain(|_, t| t.elapsed() < Duration::from_secs(5));

        match res {
            Ok(event) => {
                tracing::debug!("notify イベント: {:?} paths={:?}", event.kind, event.paths);
                handle_event(app_handle, state, event, &mut recently_processed).await;
            }
            Err(e) => {
                tracing::warn!("notify エラー: {e}");
                let _ = app_handle.emit("watcher_error", e.to_string());
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// イベント処理
// ---------------------------------------------------------------------------

async fn handle_event(
    app_handle: &AppHandle,
    state: &AppState,
    event: Event,
    recently_processed: &mut HashMap<PathBuf, Instant>,
) {
    if !state.is_enabled.load(Ordering::Relaxed) {
        tracing::debug!("翻訳 OFF のためスキップ: {:?}", event.kind);
        return;
    }

    let is_relevant = matches!(
        event.kind,
        EventKind::Create(_)
        | EventKind::Modify(ModifyKind::Name(RenameMode::To | RenameMode::Any))
        | EventKind::Modify(ModifyKind::Data(_))
        | EventKind::Any
    );

    if !is_relevant {
        return;
    }

    for path in event.paths {
        if !is_png(&path) {
            continue;
        }
        if recently_processed.contains_key(&path) {
            tracing::debug!("重複スキップ: {:?}", path);
            continue;
        }
        recently_processed.insert(path.clone(), Instant::now());

        tracing::info!("PNG 検出: {}", path.display());

        // ファイルの書き込みが完了するまで少し待つ
        sleep(Duration::from_millis(200)).await;

        if let Err(e) = process_screenshot(app_handle, state, path).await {
            tracing::error!("翻訳パイプラインエラー: {e}");
            let _ = app_handle.emit("watcher_error", e.to_string());
        }
    }
}

async fn process_screenshot(
    app_handle: &AppHandle,
    state: &AppState,
    path: PathBuf,
) -> anyhow::Result<()> {
    let (prompt, osc_config, osc_prefix_enabled, chunk_interval_secs) = {
        let config = state.config.lock().expect("config lock poisoned");
        (config.translation_prompt.clone(), config.osc.clone(), config.osc_prefix_enabled, config.osc.chunk_interval_secs)
    };
    let osc_enabled = state.osc_enabled.load(Ordering::Relaxed);
    let translator = state.get_translator();

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut guard = state.cancel_sender.lock().expect("cancel lock poisoned");
        *guard = Some(cancel_tx);
    }

    tracing::info!("翻訳開始: {}", path.display());
    let _ = app_handle.emit("translation_start", path.to_string_lossy().as_ref());

    let translate_result = tokio::select! {
        r = translator.translate(&path, &prompt) => Some(r),
        _ = cancel_rx => None,
    };

    // cancel_sender をクリア（translate が先に終わった場合も含む）
    {
        let mut guard = state.cancel_sender.lock().expect("cancel lock poisoned");
        *guard = None;
    }

    let translated_text = match translate_result {
        Some(r) => r?,
        None => {
            tracing::info!("翻訳がキャンセルされました: {}", path.display());
            let _ = app_handle.emit("translation_cancelled", ());
            return Ok(());
        }
    };
    tracing::info!("翻訳完了: {:?}", translated_text);

    // サムネイル生成（失敗してもエントリ自体は作成する）
    let thumbnail_path = crate::image_utils::generate_thumbnail(path.clone()).await
        .map_err(|e| tracing::warn!("サムネイル生成に失敗しました: {e}"))
        .ok();

    let entry = TranslationEntry {
        timestamp: Utc::now(),
        image_path: path,
        translated_text: translated_text.clone(),
        provider: translator.name().to_string(),
        model: translator.model_name().to_string(),
        thumbnail_path,
    };
    state.push_history(entry.clone());
    // 翻訳完了をフロントに通知（OSC送信より先に行い、UIにすぐ全文を表示する）
    app_handle.emit("translation_done", &entry)?;

    if osc_enabled {
        let handle = app_handle.clone();
        tokio::spawn(async move {
            let chunks = osc::split_for_osc(&translated_text, osc_prefix_enabled);
            let total = chunks.len();
            for (i, chunk) in chunks.iter().enumerate() {
                let current = i + 1;
                if let Err(e) = osc::send_to_chatbox(&osc_config, chunk) {
                    tracing::error!("OSC 送信エラー (チャンク {}/{}): {e}", current, total);
                    let _ = handle.emit("watcher_error", e.to_string());
                    return;
                }
                if total > 1 {
                    let _ = handle.emit("osc_chunk_progress", OscChunkProgress { current, total });
                }
                if current < total {
                    sleep(Duration::from_secs(chunk_interval_secs)).await;
                }
            }
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ヘルパー
// ---------------------------------------------------------------------------

fn is_png(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
}
