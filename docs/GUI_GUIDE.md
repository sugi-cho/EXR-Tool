# ExrTool GUI 操作ガイド

本ガイドは Tauri ベースの GUI 版 ExrTool の基本操作をまとめています。初回セットアップは docs/BOOTSTRAP.md を参照してください。

- 対象バージョン: 2025-09-14 時点の master
- 参考画像: docs/img/ 配下（差し替え推奨）

## 起動と基本
- 起動コマンド
  - `cd apps/exrtool-gui/src-tauri`
  - `cargo tauri dev`
  - 画面構成（例）
  - ファイルパス入力欄と「開く」ボタン（EXRを選択）
  - プレビューキャンバス（PNGに準ずる見た目）
  - 調整パネル: `Max Size`, `Exposure`, `Gamma`, `High Quality`（HQリサイズ）
  - LUT関連: 外部 `.cube` 指定／プリセット選択／メモリ内LUTのON/OFF
  - スコープ: ヒストグラム＆波形（`Channel`/`Scale` 切り替え可能）
  - 情報表示: `preview: WxH`、ログ表示ボタン

参考: ![メイン画面の例](img/ui_main.png)

## 操作フロー（クイックスタート）
1) EXR を開く
   - 「参照」→ EXR を選択 → `open_exr` 実行 → プレビュー生成
2) 露出/ガンマ
   - `Exposure` と `Gamma` を調整（入力はデバウンス済み）
   - HQ表示: `High Quality` をON（Lanczosリサイズ）
3) LUT適用
   - 外部LUT: `.cube` を指定 → `Use LUT(in-memory)` をON
   - プリセット: ドロップダウンから選択（例: `acescg(linear) -> srgb(srgb)`）
   - プリセット反映後は自動で in-memory LUT が有効化
4) ピクセル検査（スポイト）
   - プレビューをマウス移動 → ステータス欄にリニアRGBA表示
   - クリックで値を固定＆クリップボードにコピー（もう一度クリックで解除）
5) PNG 書き出し
   - 「保存」→ 出力パスを指定 → 現在のプレビューをPNG保存

## スコープ表示（ヒストグラム／波形）
- プレビュー更新後に自動計算され、キャンバス下に表示されます。
- `Channel` で `RGB`/`R`/`G`/`B` を切り替え。
- `Scale` で縦方向の表示倍率を変更。

## Video Tools（連番EXR）

前提: `cargo tauri dev -- -F exr_pure` でGUIを起動してください（EXRメタデータ機能が有効化されます）。

1) Set FPS（属性一括付与）
   - Sequence Folder を選択 → FPS値と Attribute（既定: `FramesPerSecond`）を設定 → Apply FPS。
   - Dry Run: 対象件数のみ計算し、プログレスは即100%になります。
   - 実行時の安全性:
     - 書き換え前に `*.exr.bak` を作成します。
     - 書き込みは一時ファイルへ全出力→置換（Windowsは既存削除→rename）方式です。
     - 全件成功時のみバックアップを自動削除。失敗があればバックアップは保持します。
   - 進捗表示: 進捗イベント `seq-progress` を受けてバーが0→100%で更新されます。
     - 長時間処理でもUIが固まらないよう、バックグラウンド実行＋更新スロットリング（約100ms/0.5%）を適用しています。

2) Export ProRes（EXR連番→MOV）
   - 依存: `ffmpeg` が PATH に必要です。
   - Colorspace を選択（`linear:srgb` / `acescg:srgb` / `aces2065:srgb`）。
   - 「Export ProRes」実行でプログレスバー（`video-progress`）が進行します。

## メタデータ（任意機能）
- feature `use_exr_crate` を有効にすると、属性テーブルが有効化されます
  - 読み込み: 画面内の属性テーブルへ反映
  - 編集: セルを直接編集／行追加・削除（差分ハイライト）
  - 書き出し: 別名での非破壊保存（将来アトミック置換対応予定）

## 一括適用（CLI 連携）
- ルール定義（docs/rules.yml など）を用意し、CLI で一括出力できます
  - 例: `cargo run -p exrtool-cli -- apply --rules docs/rules.yml --dry-run false --backup true`
  - ルールは `input/output/max_size/exposure/gamma/lut` を指定可能

## ショートカット/操作の豆知識
- 調整入力は一定時間（約120ms）でデバウンス → ラグの少ない更新
- `High Quality` は大きな画像でのプレビュー品質向上に有効（負荷は上がります）
- LUTプリセットは `config/luts.presets.json` から読み込まれます（カスタム可）

## トラブルシューティング
- 「WebView2 が見つからない」
  - `winget install -e --id Microsoft.EdgeWebView2Runtime` を実行
- 「ビルドに失敗（MSVC/SDK）」
  - VS 2022 Build Tools（C++/Windows SDK）を導入。bootstrap スクリプト再実行
- 「プレビューが更新されない」
  - `High Quality` ON時は更新に時間がかかる場合があります。ログを開いてエラー有無を確認
- 「LUTが反映されない」
  - 外部 `.cube` は読み込みエラー時にログへ記録。プリセットでの再現も試してください
- 「Video Tools 実行中にウィンドウが固まる」
  - バックグラウンド化と進捗スロットリングで改善済みです。もし再現する場合は対象件数とログ（`apps/exrtool-gui/src-tauri/exrtool-gui.log`）を添えて報告してください。

## スクリーンショットの差し替え
- 画像は `docs/img/` に配置してください。
  - 例: `docs/img/ui_main.png`, `docs/img/ui_lut_preset.png`
- 実アプリのスクリーンショットで差し替えるとユーザー理解が高まります

---
このガイドの改善提案や画像提供は大歓迎です。PR テンプレートに沿ってご提出ください。
