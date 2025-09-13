# 開発プロセスまとめ（Codex × リポジトリ運用）

目的: 私（人間）と Codex のやり取りで確立した開発手順を共有し、次の人が迷わず同じ運用で進められるようにする。AGENTS.md の最小ルールを前提に、実際の運用ノウハウを補足する。

## 役割と基本方針
- 人間: 要求/優先度/再現手順を提示、ログを提供。
- Codex: 原因特定→最小修正→検証→記録→コミット。
- 原則: 失敗→原因特定→最小修正→検証を高速反復（最大5回）。副作用の大きい変更は分割。
- 参照: AGENTS.md（全体ルール）、docs/BOOTSTRAP.md（環境）、docs/TASKS.md（唯一のタスクソース）。

## ブートストラップ（Windows）
- PowerShell（管理者）で実行:
  - `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`
  - `./scripts/bootstrap_windows.ps1`
- 役割: Git/Rust/VSBuildTools/WebView2/`tauri-cli` を自動導入。`rustup`/`cargo` PATH 未反映にも対応。

## 反復サイクル（再掲＋実践Tips）
1) 事実確認: 直近の失敗ログを `log/error.log` に貼る（先頭に実行コマンド）。
2) 最小修正: 影響範囲を見極めてピンポイントにパッチ。
3) 検証: 
   - CLI: `cargo build -p exrtool-cli`
   - GUI: `cd apps/exrtool-gui/src-tauri && cargo build`
4) 続報: 直らなければ最大5回まで繰り返し。（超える場合は一旦停止し合意）
5) 記録/同期: `log/YYYY-MM-DD-notes.md` を更新、必要なら README/docs を同期。
6) コミット: Conventional Commits。1コミット=1まとまり。

### ログ運用テンプレート
```
# 実行したコマンド
cargo build -p exrtool-cli

# エラーログ
<全文>
```
（注）機密情報やPIIは貼らない。必要なら伏字化。

## ブランチ/PR運用（Codex Cloud → Codex CLI）
- Cloud（実装）: タスク単位（1タスク=1PR）。PRには目的/実装/検証/影響範囲を記載。
- CLI（集約）: docs/TASKS.mdの優先度に基づき、PRを順にマージ。
  - 一連の流れ（ scripts/merge_assist.ps1 が自動化）
    1) `gh pr checkout <#>`
    2) クイックビルド: `cargo check -p exrtool-cli` + `apps/.../src-tauri` で `cargo check`
    3) 競合時: `git merge -s ort -X theirs origin/master`（PRの意図を優先）→ `git push`
    4) マージ: `gh pr merge <#> -m -d`（失敗時 `--auto` を試行）
  - マージ後: ローカルのマージ済みブランチを削除（`git branch -d`）。未マージ扱いはローカル専用コミットが原因のことがある（`-D`で強制削除可）。

## CIガード
- GitHub Actions（.github/workflows/build.yml）
  - lint: `rustfmt --check` / `clippy -D warnings`
  - feature: `exrtool-core --features use_exr_crate` で `cargo check`
  - build: CLI/GUI を 3OS マトリクスでビルド
- 推奨: ブランチ保護で lint/build を Required に設定。

## スクリプト（ローカル自動化）
- マージ支援: `scripts/merge_assist.ps1`
  - 例: 全PR → `./scripts/merge_assist.ps1`
  - 例: 指定PR → `./scripts/merge_assist.ps1 -Numbers 25,26,27`
- 簡易CI: `scripts/ci_local.ps1`
  - 実行: `./scripts/ci_local.ps1`
  - 実施: fmt/clippy/build、`use_exr_crate`チェック

## 機能フラグと依存
- `use_exr_crate`: EXRメタデータ（`exr`クレート）。
- `use_ocio`（実験的）: OpenColorIO FFI。環境依存が大きいため既定OFF。build.rsは feature 無効時にスキップ。

## よくあるつまずきと対処
- `rustup`/`cargo` が見つからない: bootstrap が PATH を補う。PS再起動で解決する場合あり。
- PowerShell で `||` が使えない: `if (!$?) { ... }` で分岐。
- 競合解消後にマージ不可: いったん `git push origin HEAD` 後、`gh pr merge -m -d` を再実行。
- build.rs で bindgen/clang 依存エラー: `use_ocio` を無効（既定）。必要時にのみ環境構築して有効化。

## 実績（2025-09-13）
- PR #3〜#32 を優先度順にマージ、競合解消。
- ビルド修正（core/cli/gui、bootstrap改善）。
- docs 更新（TASKS/README/BOOTSTRAP）。
- devtools 追加（merge_assist.ps1 / ci_local.ps1）、CIガード強化。

---
本ドキュメントはプロセスの“要約”。詳細は `AGENTS.md` と `docs/TASKS.md`、日次ノート `log/YYYY-MM-DD-notes.md` を参照。

