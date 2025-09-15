# ExrTool GUI 操作ガイド

本ガイドは Tauri ベースの GUI 版 ExrTool の基本操作をまとめています。初回セットアップは docs/BOOTSTRAP.md を参照してください。

- 対象バージョン: 2025-09-14（UI簡素化後）
- 参考画像: docs/img/ 配下（差し替え推奨）

## 起動と基本
- 起動コマンド
  - `cd apps/exrtool-gui/src-tauri`
  - `cargo tauri dev`
  - 画面構成（例）
  - ファイルパス入力欄と「開く」ボタン（EXRを選択）
  - プレビューキャンバス（PNGに準ずる見た目）
  - ヘッダー: パス入力＋「開く」、Transform選択、PNG保存、ログ表示
  - HQ/LUTのUIは廃止（どちらも既定ON）。Transform選択で自動適用されます。
  - スコープ: ヒストグラム＆波形（`Channel`/`Scale` 切り替え可能）
  - 情報表示: `preview: WxH`、ログ表示ボタン
  - 設定: 匿名ログ送信を許可するオプション（既定はOFF）

参考: ![メイン画面の例](img/ui_main.png)

## 操作フロー（クイックスタート）
1) EXR を開く
   - 「参照」→ EXR を選択 → `open_exr` 実行 → プレビュー生成
2) Transform適用
   - ドロップダウンからTransformを選ぶと自動で in-memory LUT を適用し、プレビューが更新されます。
   - 外部 `.cube` の読み込みはGUIでは廃止しました（CLIでは利用可能）。
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
    - キャンセル: 処理中は「Cancel」ボタンで中断できます。キャンセルするとバックアップファイルは削除され、進捗バーもリセットされます。

2) Export ProRes（EXR連番→MOV）
   - 依存: `ffmpeg` が PATH に必要です。
   - Colorspace を選択（`linear:srgb` / `acescg:srgb` / `aces2065:srgb`）。
   - 「Export ProRes」実行でプログレスバー（`video-progress`）が進行します。

## メタデータ（任意機能）
- feature `use_exr_crate` を有効にすると、属性テーブルが表示されます
  - 読み込み: 画面内の属性テーブルへ反映
  - 編集: 現在はサポートされておらず、閲覧のみ可能です

## 一括適用（CLI 連携）
- ルール定義（docs/rules.yml など）を用意し、CLI で一括出力できます
  - 例: `cargo run -p exrtool-cli -- apply --rules docs/rules.yml --dry-run false --backup true`
  - ルールは `input/output/max_size/exposure/gamma/lut` を指定可能

## ショートカット/操作の豆知識
- プレビュー更新はデバウンス（約120ms）で滑らかに適用されます。
- Transformプリセットは `config/transforms.json`（存在しない場合は既定）から読み込まれます。

## トラブルシューティング
- 「WebView2 が見つからない」
  - `winget install -e --id Microsoft.EdgeWebView2Runtime` を実行
- 「ビルドに失敗（MSVC/SDK）」
  - VS 2022 Build Tools（C++/Windows SDK）を導入。bootstrap スクリプト再実行
- 「プレビューが更新されない」
  - Transform変更で自動適用されない場合、ログ表示からエラー有無を確認してください。
  - まれに Tauri API 解決が遅延する場合があります。アプリを再起動、または少し待ってから再操作してください。
- 「Video Tools 実行中にウィンドウが固まる」
  - バックグラウンド化と進捗スロットリングで改善済みです。もし再現する場合は対象件数とログ（`apps/exrtool-gui/src-tauri/exrtool-gui.log`）を添えて報告してください。

## スクリーンショットの差し替え
- 画像は `docs/img/` に配置してください。
  - 例: `docs/img/ui_main.png`, `docs/img/ui_lut_preset.png`
- 実アプリのスクリーンショットで差し替えるとユーザー理解が高まります

---
このガイドの改善提案や画像提供は大歓迎です。PR テンプレートに沿ってご提出ください。
