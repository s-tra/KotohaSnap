use std::io::Cursor;
use std::path::{Path, PathBuf};
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

// ---------------------------------------------------------------------------
// サムネイルキャッシュの古いファイルを削除
// ---------------------------------------------------------------------------

/// 起動時に呼び出し、`days` 日以上アクセスされていないサムネイルを削除する。
pub fn cleanup_old_thumbnails(days: u64) {
    let thumb_dir = match dirs::cache_dir() {
        Some(d) => d.join("vrc-translator").join("thumbnails"),
        None => return,
    };
    if !thumb_dir.exists() {
        return;
    }

    let entries = match std::fs::read_dir(&thumb_dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("サムネイルディレクトリの読み取りに失敗: {e}");
            return;
        }
    };

    let threshold = std::time::Duration::from_secs(days * 24 * 60 * 60);
    let mut deleted = 0u32;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
            continue;
        }
        let accessed = entry.metadata()
            .and_then(|m| m.accessed().or_else(|_| m.modified()))
            .and_then(|t| t.elapsed().map_err(|e| std::io::Error::other(e)));

        match accessed {
            Ok(age) if age > threshold => {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("サムネイルの削除に失敗: {} ({e})", path.display());
                } else {
                    deleted += 1;
                }
            }
            _ => {}
        }
    }

    if deleted > 0 {
        tracing::info!("古いサムネイルを {deleted} 件削除しました（{days} 日以上未アクセス）");
    }
}

// ---------------------------------------------------------------------------
// サムネイル生成
// ---------------------------------------------------------------------------

/// スクリーンショットから 160×120px 以内の JPEG サムネイルを生成してパスを返す。
/// キャッシュ先: OS の cache_dir / vrc-translator / thumbnails / <stem>.jpg
pub async fn generate_thumbnail(image_path: PathBuf) -> Result<PathBuf> {
    let thumb_dir = dirs::cache_dir()
        .context("OS のキャッシュディレクトリが取得できませんでした")?
        .join("vrc-translator")
        .join("thumbnails");

    std::fs::create_dir_all(&thumb_dir)
        .with_context(|| format!("サムネイルディレクトリの作成に失敗: {}", thumb_dir.display()))?;

    let stem = image_path
        .file_stem()
        .context("ファイル名の取得に失敗")?
        .to_string_lossy()
        .into_owned();
    let thumb_path = thumb_dir.join(format!("{stem}.jpg"));

    // すでに生成済みならスキップ
    if thumb_path.exists() {
        return Ok(thumb_path);
    }

    let thumb_path_clone = thumb_path.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let raw = std::fs::read(&image_path)
            .with_context(|| format!("画像の読み込みに失敗: {}", image_path.display()))?;
        let img = image::load_from_memory(&raw)
            .context("画像のデコードに失敗")?;

        // 160×120 以内にリサイズ（アスペクト比保持）
        let thumb = img.thumbnail(160, 120);

        let file = std::fs::File::create(&thumb_path_clone)
            .with_context(|| format!("サムネイルファイルの作成に失敗: {}", thumb_path_clone.display()))?;
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            std::io::BufWriter::new(file),
            75,
        );
        thumb.write_with_encoder(encoder)
            .context("サムネイルの JPEG エンコードに失敗")?;

        Ok(())
    })
    .await
    .context("サムネイル生成タスクの実行に失敗")??;

    Ok(thumb_path)
}

fn mime_from_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif")                => "image/gif",
        Some("webp")               => "image/webp",
        _                          => "image/png",
    }
}
