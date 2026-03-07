use std::io::Cursor;
use std::path::Path;
use anyhow::{Context, Result};

/// 2 MB 超は圧縮してアップロードサイズを削減
const MAX_BYTES: usize = 2 * 1024 * 1024;
/// リサイズ後の最大辺長（ピクセル）
const MAX_DIM: u32 = 1920;

/// 画像ファイルを読み込み、API に送れる形に整えて (bytes, mime_type) で返す。
///
/// - 5 MB 以下の PNG → そのまま返す
/// - 5 MB 超 → 最大辺 1920px にリサイズして JPEG(q=85) で再エンコード
pub async fn load_and_prepare(path: &Path) -> Result<(Vec<u8>, &'static str)> {
    let raw = tokio::fs::read(path)
        .await
        .with_context(|| format!("画像の読み込みに失敗しました: {}", path.display()))?;

    if raw.len() <= MAX_BYTES {
        let mime = mime_from_path(path);
        return Ok((raw, mime));
    }

    tracing::info!(
        "画像が {}MB 超のためリサイズします: {}",
        raw.len() / 1024 / 1024,
        path.display()
    );

    // tokio のスレッドプールで CPU バウンドな処理を実行
    let compressed = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let img = image::load_from_memory(&raw)
            .context("画像のデコードに失敗しました")?;

        // 最大辺を MAX_DIM に収める（小さい場合はそのまま）
        let img = if img.width() > MAX_DIM || img.height() > MAX_DIM {
            img.resize(MAX_DIM, MAX_DIM, image::imageops::FilterType::Triangle)
        } else {
            img
        };

        // JPEG q=85 でエンコード
        let mut buf = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            Cursor::new(&mut buf),
            85,
        );
        img.write_with_encoder(encoder)
            .context("JPEG エンコードに失敗しました")?;

        tracing::info!("リサイズ後サイズ: {} KB", buf.len() / 1024);
        Ok(buf)
    })
    .await
    .context("画像処理タスクの実行に失敗しました")??;

    Ok((compressed, "image/jpeg"))
}

fn mime_from_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif")                => "image/gif",
        Some("webp")               => "image/webp",
        _                          => "image/png",
    }
}
