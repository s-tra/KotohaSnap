use std::net::UdpSocket;
use anyhow::{Context, Result};
use rosc::{encoder, OscMessage, OscPacket, OscType};

use crate::config::OscConfig;

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
