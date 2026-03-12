use std::net::UdpSocket;
use anyhow::{Context, Result};
use rosc::{encoder, OscMessage, OscPacket, OscType};

use crate::config::OscConfig;

/// VRChat チャットボックスの最大文字数（Unicode コードポイント単位）。
/// pub const CHATBOX_MAX_CHARS: usize = 144;
pub const CHATBOX_MAX_CHARS: usize = 120;

/// OSC チャンク1件のコンテンツ部分の目標文字数。
/// CHATBOX_MAX_CHARS より小さく設定することで、テキスト量にかかわらず
/// VRChat の表示領域に対して常に一定の余裕を確保する。
const OSC_CONTENT_TARGET: usize = 90;

/// OSC 送信用にテキストを分割してチャンクのリストを返す。
///
/// - `prefix_enabled = false`: プレフィックスなしで 144 文字ごとに分割
/// - `prefix_enabled = true` かつ 1 チャンクで収まる場合: `[翻訳結果]\n{text}`
/// - `prefix_enabled = true` かつ複数チャンクに分割される場合:
///   各チャンクの先頭に `[翻訳結果{i}/{n}]\n` を付与
///
/// チャンク数とプレフィックス長の相互依存は収束ループで解決する。
pub fn split_for_osc(text: &str, prefix_enabled: bool) -> Vec<String> {
    // VRChat のチャットボックスは表示行数が限られているため、
    // 連続する改行（空行）を1つの改行に圧縮する。
    let normalized: String = {
        let mut s = String::with_capacity(text.len());
        let mut prev_nl = false;
        for c in text.chars() {
            if c == '\n' {
                if !prev_nl { s.push(c); }
                prev_nl = true;
            } else {
                prev_nl = false;
                s.push(c);
            }
        }
        s
    };
    let text = normalized.trim();

    let chars: Vec<char> = text.chars().collect();

    if !prefix_enabled {
        if chars.len() <= OSC_CONTENT_TARGET {
            return vec![text.to_string()];
        }
        // OSC_CONTENT_TARGET を上限として均等分割
        let total = (chars.len() + OSC_CONTENT_TARGET - 1) / OSC_CONTENT_TARGET;
        let content_size = (chars.len() + total - 1) / total;
        return chars
            .chunks(content_size)
            .map(|c| c.iter().collect())
            .collect();
    }

    // プレフィックスありの場合
    // まず 1 チャンクで収まるか確認（"[翻訳結果]\n" = 7 文字）
    const SINGLE_PREFIX: &str = "[翻訳結果]\n";
    let single_prefix_len = SINGLE_PREFIX.chars().count();
    if chars.len() + single_prefix_len <= CHATBOX_MAX_CHARS {
        return vec![format!("{}{}", SINGLE_PREFIX, text)];
    }

    // 複数チャンクに分割: "[翻訳結果{i}/{n}]\n" プレフィックス付き
    // OSC_CONTENT_TARGET を目標コンテンツ文字数として分割数を決める
    let total = (chars.len() + OSC_CONTENT_TARGET - 1) / OSC_CONTENT_TARGET;

    // total チャンクに均等分割できる content_size を算出
    // ただし prefix + content_size <= CHATBOX_MAX_CHARS を守る
    let sample = format!("[翻訳結果{t}/{t}]\n", t = total);
    let prefix_len = sample.chars().count();
    let max_content = CHATBOX_MAX_CHARS.saturating_sub(prefix_len);
    let content_size = ((chars.len() + total - 1) / total)
        .min(max_content)
        .max(1);

    chars
        .chunks(content_size)
        .enumerate()
        .map(|(i, c)| {
            let content: String = c.iter().collect();
            format!("[翻訳結果{}/{}]\n{}", i + 1, total, content)
        })
        .collect()
}

/// 翻訳テキストを VRChat チャットボックスに OSC 送信する。
///
/// VRChat OSC チャットボックス仕様:
///   アドレス : `/chatbox/input`（config.address で上書き可）
///   引数     : [String(text), Bool(immediate), Bool(notification)]
///     - immediate   : true = キーボードアニメーションをスキップ
///     - notification: true = 通知音を鳴らす
pub fn send_to_chatbox(config: &OscConfig, text: &str) -> Result<()> {
    let packet = OscPacket::Message(OscMessage {
        addr: config.address.clone(),
        args: vec![
            OscType::String(text.to_string()),
            OscType::Bool(true),  // immediate
            OscType::Bool(false), // notification
        ],
    });

    let bytes = encoder::encode(&packet)
        .context("OSC パケットのエンコードに失敗しました")?;

    // 送信元は OS に任せる（0.0.0.0:0）
    let socket = UdpSocket::bind("0.0.0.0:0")
        .context("UDP ソケットのバインドに失敗しました")?;

    let dest = format!("{}:{}", config.host, config.port);
    socket
        .send_to(&bytes, &dest)
        .with_context(|| format!("OSC パケットの送信に失敗しました (宛先: {dest})"))?;

    tracing::info!("OSC 送信: {} -> {:?}", dest, text);
    Ok(())
}

/// OSC 疎通確認用のテスト送信。
/// `commands.rs` の `test_osc` コマンドから呼び出す。
pub fn test_send(config: &OscConfig) -> Result<()> {
    send_to_chatbox(config, "OSC test")
}
