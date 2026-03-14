# KotohaSnap

VRChat のスクリーンショットを自動検出し、テキストを翻訳して OSC 経由でチャットボックスに送信するデスクトップアプリです。

## 機能

- VRChat のスクリーンショット保存フォルダを参照し、新しい `.png` を自動検出
- 検出した画像をビジョン対応 AI モデルで翻訳
- 翻訳結果を OSC 経由で VRChat チャットボックスにリアルタイム送信
- 長文は自動で分割して順番に送信
- 翻訳ログをアプリ内に表示（サムネイル付き）

## 動作環境

- Windows 11

## 対応 AI プロバイダー

| プロバイダー | 備考                                         |
|---|--------------------------------------------|
| Google (Gemini) | デフォルトモデル: `gemini-flash-latest`            |
| OpenAI (GPT) | デフォルトモデル: `gpt-4o`                         |
| Anthropic (Claude) | デフォルトモデル: `claude-haiku-4-5-20251001`      |
| Groq (LLaMA) | デフォルトモデル: `llama-4-scout-17b-16e-instruct` |
| カスタム (OpenAI 互換) | ローカルLLM等に対応                                |

## インストール

[配布ページ](https://github.com/s-tra/KotohaSnap/releases) からインストーラー（`.msi` または `.exe`）をダウンロードして実行してください。

## セットアップ

1. アプリを起動
2. 右上の「設定」ボタンから設定を開く
3. **プロバイダー設定** で使用するプロバイダーを選択し、 API キーを入力
   - API キーは、利用者自身で用意してください
4. **スクリーンショットフォルダ** に VRChat の画像保存先を指定
   - デフォルト: `%USERPROFILE%\Pictures\VRChat`
5. 「設定を保存」をクリック
6. VRChat の設定で OSC を有効化（`Action Menu → Options → OSC → Enable`）



## 使い方

### メインウィンドウ

基本的に、KotohaSnapを起動した状態でVRChat上で写真を撮るだけです。

| 要素 | 説明 |
|---|---|
| 翻訳スイッチ | ON にするとスクリーンショットの自動翻訳を開始 |
| チャット送信スイッチ | ON にすると翻訳結果を VRChat チャットボックスに OSC 送信 |
| ログリスト | 翻訳履歴をリアルタイム表示（最大 200 件） |
| フォントサイズ − / ＋ | ログの文字サイズを変更（10〜20px） |
| クリア | 翻訳ログを全削除 |

### 翻訳中の操作

- 翻訳処理中は「翻訳中...」カードが表示されます。**キャンセル**ボタンで中断できます。
- OSC 送信中は「OSC送信中 (N/M)」バーが表示されます。**送信を中止**ボタンで送信中断できます（送信済みのチャンクは取り消しできません）。

### OSC 送信の仕様

- VRChat チャットボックスの文字数制限を超える長文は、自動で 90 文字ごとに分割して送信
- チャンク間の送信間隔はデフォルト 4 秒（設定画面で変更可能）
- プレフィックス ON の場合: `[翻訳結果]` または `[翻訳結果1/n]` を先頭に付与

## 設定項目

| 項目               | 説明                       | デフォルト               |
|------------------|--------------------------|---------------------|
| スクリーンショットフォルダ    | 監視するフォルダ                 | `~/Pictures/VRChat` |
| 翻訳完了時に通知音を鳴らす    | 翻訳完了時のビープ音               | ON                  |
| OSC ホスト          | VRChat が動作している PC の IP   | 127.0.0.1           |
| OSC ポート          | VRChat の OSC 受信ポート       | 9000                |
| OSC アドレス         | OSC メッセージの送信先アドレス        | /chatbox/input      |
| 分割送信の間隔          | チャンク間の待機時間               | 4 秒                 |
| `[翻訳結果]` プレフィックス | チャットボックスに表示するプレフィックス     | ON                  |
| プロバイダー           | 翻訳に使用する AI プロバイダー        | groq                |
| api キー           | 各 AI プロバイダーに対応した api キー | (空欄)                |
| 翻訳プロンプト          | API に渡すシステムプロンプト         | (日本語翻訳用)            |

設定ファイルの保存先: `%APPDATA%\kotoha-snap\config.toml`

## ビルド方法

### 必要なもの

- [Rust](https://rustup.rs/) 1.77.2 以上
- [Tauri CLI v2](https://tauri.app/start/prerequisites/)
- [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (Windows)

### 手順

```bash
# リポジトリをクローン
git clone https://github.com/s-tra/KotohaSnap.git
cd KotohaSnap

# 開発サーバー起動
cargo tauri dev

# リリースビルド
cargo tauri build
```

## 技術スタック

| 役割 | 技術 |
|---|---|
| バックエンド | Rust + Tauri v2 |
| フロントエンド | Vanilla JS / HTML / CSS（フレームワークなし） |
| 非同期ランタイム | tokio |
| HTTP クライアント | reqwest |
| ファイル監視 | notify |
| OSC 送信 | rosc |
| 設定保存 | serde + toml |
| 画像処理 | image crate |


## ライセンス

MIT License — Copyright (c) 2026 s-tra
