# AGENTS.md — Agent/開発者向け運用ルール（ExrTool）

このファイルは、本リポジトリでエージェント（AI）と人間が協働するための最小ルールを定めます。スコープはリポジトリ全体です。

## 目的
- 失敗→原因特定→最小修正→検証の反復を素早く回す。
- ログの置き場とやり取りの約束を明確にする。
- 変更は小さく、関連箇所に限定し、コミットを意味のある単位に保つ。

## 役割
- 人間: 要求・再現手順・エラーログを提供する。優先順位を決める。
- エージェント: ログを読み、原因を特定し、最小パッチを提案・適用・検証する。

## ログ運用
- ターミナル/ビルド/実行エラーは「直近の 1 回分」を `log/error.log` に貼り付ける。
  - 先頭に実行したコマンド行を含めること（例: `cargo run ...`）。
  - 次の反復時は上書きして構わない（履歴は日付ノートへ）。
- GUI（Tauri）側の実行ログ:
  - ファイル: `apps/exrtool-gui/src-tauri/exrtool-gui.log`（ウィンドウを閉じても残る）。
  - 画面の「ログ表示」ボタンからも取得可能。
- 日次サマリ: `log/YYYY-MM-DD-notes.md` に主要作業・決定・未解決事項を追記。

## 反復サイクル（エージェント）
1) `log/error.log` を読む（最優先の事実）。
2) 影響範囲を推定し、最小パッチを用意して適用。
3) 検証:
   - CLI: `cargo run -p exrtool-cli -- ...` / `cargo build -p exrtool-cli`
   - GUI: `cd apps/exrtool-gui/src-tauri && cargo build`（必要に応じ `cargo tauri dev`）
4) 直らない場合は最大 5 回まで反復（それ以上は一旦停止し要相談）。
5) `log/YYYY-MM-DD-notes.md` を更新（何を直し、次に何をするか）。
6) Git へコミット（意味のある粒度・メッセージ）。

## 変更ポリシー
- 原因に直結する最小修正に限定。副作用の大きい改修は分割。
- 設定/依存の追加は理由を明記。不要なグローバル変更は避ける。
- 既存スタイルを尊重（言語/ツールのデフォルトに揃える）。

## Git ルール（簡易）
- 初回: リポジトリ未初期化なら `git init` → 以後は通常コミット。
- メッセージ書式（例）:
  - `fix(core): explain root cause and minimal patch`
  - `feat(gui): add logging panel`
  - `docs(bootstrap): add setup guide`
- 1 コミット = 1 まとまりの変更。不要ファイルは含めない。

## Tauri/GUI の既知ポイント
- `tauri.conf.json` は v1 前提。`withGlobalTauri: true` と `allowlist.dialog` を使用。
- フロントの `index.js` は DOMContentLoaded 後に初期化。`window.__TAURI__` 未注入に備え 5 秒リトライ。
- `icons/icon.ico` は build.rs で自動生成（Windows）。差し替えたい場合は同パスに設置。

## ブートストラップ
- Windows 自動セットアップ: `scripts/bootstrap_windows.ps1`
- 詳細手順: `docs/BOOTSTRAP.md`

## セキュリティ/プライバシ
- 機密情報・個人情報は `log/error.log` に貼らない。必要なら伏字化。
- 認証情報（API キー等）をリポジトリに含めない。

## 依頼テンプレート（人間→エージェント）
```
# 実行環境/状況
OS/シェル/ブランチなど

# 実行したコマンド
<コマンド 1 行>

# エラーログ（log/error.log にも保存済み）
<貼り付け>

# 期待する結果
<簡潔に>
```

以上。
