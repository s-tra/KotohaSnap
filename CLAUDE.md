# vrc-translator

VRChat用スクリーンショット翻訳アプリ。Pythonで開発した同機能アプリをRust + Tauriで再実装したもの。

## プロジェクト概要

- VRChatのスクリーンショット保存ディレクトリを監視し、新しいスクリーンショットを検出したら自動翻訳
- 翻訳結果をOSC経由でVRChatのチャットボックスに送信
- マルチプロバイダー対応（Anthropic / OpenAI / Groq）
- 設定・ログ・ON/OFFトグルをGUIで操作

## 技術スタック

- **バックエンド**: Rust + Tauri v2
- **フロントエンド**: Vanilla JS / HTML / CSS（フレームワークなし）
- **非同期**: tokio
- **設定保存先**: OSの標準設定ディレクトリ（`dirs` crate 経由）

## 主要 crate

| 用途             | crate              |
|------------------|--------------------|
| 非同期ランタイム  | `tokio`            |
| HTTPクライアント  | `reqwest`          |
| ファイル監視      | `notify`           |
| OSC送信          | `rosc`             |
| シリアライズ      | `serde` + `toml`   |
| 設定ディレクトリ  | `dirs`             |
| ログ             | `tracing`          |
| 日時             | `chrono`           |

## ディレクトリ構成

```
vrc-translator/
├── CLAUDE.md
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs              # エントリポイント・Tauriセットアップ
│       ├── commands.rs          # tauri::command 定義（フロントとのインターフェース）
│       ├── state.rs             # AppState（共有状態）
│       ├── config.rs            # 設定の読み書き（serde + toml）
│       ├── watcher.rs           # スクリーンショット監視（notify）
│       ├── osc.rs               # OSC送信（rosc）
│       ├── history.rs           # 翻訳ログ管理
│       └── translator/
│           ├── mod.rs           # Translator trait 定義
│           ├── anthropic.rs     # Anthropic 実装
│           ├── openai.rs        # OpenAI 実装
│           └── groq.rs          # Groq 実装
└── src/                         # Vanilla JS フロントエンド
    ├── index.html
    ├── main.js
    └── styles.css
```

## コアの型設計

### Translator trait（`translator/mod.rs`）

```rust
#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate(&self, image_path: &Path, prompt: &str) -> anyhow::Result<String>;
    fn name(&self) -> &str;
}

pub enum TranslatorKind {
    Anthropic,
    OpenAI,
    Groq,
}

pub fn build_translator(kind: &TranslatorKind, api_key: &str) -> Box<dyn Translator> {
    match kind {
        TranslatorKind::Anthropic => Box::new(anthropic::AnthropicTranslator::new(api_key)),
        TranslatorKind::OpenAI    => Box::new(openai::OpenAITranslator::new(api_key)),
        TranslatorKind::Groq      => Box::new(groq::GroqTranslator::new(api_key)),
    }
}
```

### AppState（`state.rs`）

```rust
pub struct AppState {
    pub config: Mutex<Config>,
    pub translator: Mutex<Box<dyn Translator>>,
    pub is_enabled: AtomicBool,
    pub history: Mutex<VecDeque<TranslationEntry>>,
}
```

### Config（`config.rs`）

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub provider: String,           // "anthropic" | "openai" | "groq"
    pub api_keys: ApiKeys,
    pub osc: OscConfig,
    pub watch_dir: PathBuf,
    pub translation_prompt: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ApiKeys {
    pub anthropic: String,
    pub openai: String,
    pub groq: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OscConfig {
    pub host: String,    // デフォルト: "127.0.0.1"
    pub port: u16,       // デフォルト: 9000
    pub address: String, // デフォルト: "/chatbox/input"
}
```

### TranslationEntry（`history.rs`）

```rust
#[derive(Serialize, Clone)]
pub struct TranslationEntry {
    pub timestamp: DateTime<Utc>,
    pub image_path: PathBuf,
    pub translated_text: String,
    pub provider: String,
}
```

## イベントフロー

```
[ファイルシステム]
    ↓ notify が新スクリーンショットを検出
[watcher.rs]
    ↓ AppState.is_enabled を確認（false なら無視）
[translator/*.rs]
    ↓ 画像をBase64エンコードしてAPIへ送信 → 翻訳テキストを受け取る
[osc.rs]
    ↓ 翻訳結果を OSC で VRChat に送信
[history.rs]
    ↓ TranslationEntry を VecDeque に追記（上限: 200件）
[tauri::emit("translation_done", entry)]
    ↓
[main.js] → ログUIにリアルタイム追加表示
```

## Tauri コマンド一覧（`commands.rs`）

フロントエンドから `invoke()` で呼び出す：

| コマンド                      | 説明                         |
|-------------------------------|------------------------------|
| `get_config()`                | 現在の設定を取得             |
| `save_config(config)`         | 設定を保存・即時反映         |
| `set_enabled(enabled: bool)`  | 翻訳のON/OFFを切り替え       |
| `get_history()`               | 翻訳ログ一覧を取得           |
| `clear_history()`             | ログをクリア                 |
| `test_osc()`                  | OSC送信テスト（疎通確認）    |

バックエンドからフロントへのプッシュイベント：

| イベント名          | ペイロード           | タイミング             |
|---------------------|----------------------|------------------------|
| `translation_done`  | `TranslationEntry`   | 翻訳完了時             |
| `watcher_error`     | `String`（エラー文）  | ファイル監視エラー発生時 |

## コーディング規約

- `unwrap()` は使わず `?` 演算子 または `anyhow::Result` を使う
- 共有状態へのアクセスは必ず `Mutex` 経由。デッドロックに注意（ロック保持中に await しない）
- Tauri コマンドのエラーは `Result<T, String>` で返し、フロントに伝える
- 設定ファイルの保存先は `dirs::config_dir()` を使い、OS標準のパスに従う
- ログは `tracing` マクロ（`info!`, `warn!`, `error!`）を使う

## やらないこと（スコープ外）

- 音声認識・音声翻訳
- VRChat以外のOSCターゲット対応
- 翻訳履歴のファイルへのエクスポート（将来対応候補）
- モバイル対応

## 実装の優先順位

1. `config.rs` + `state.rs`（土台）
2. `translator/mod.rs` + `translator/anthropic.rs`（まずAnthropicのみ動かす）
3. `watcher.rs`（ファイル監視ループ）
4. `osc.rs`（OSC送信）
5. `commands.rs`（フロントとのIF）
6. フロントエンドUI（`src/`）
7. `translator/openai.rs` + `translator/groq.rs`（追加プロバイダー）
