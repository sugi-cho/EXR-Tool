# ExrTool — Rust core + Tauri GUI

EXR の高速プレビュー／LUT適用／簡易編集ツールです。Rust コア（`exrtool-core`）と Tauri GUI（`exrtool-gui`）、CLI（`exrtool-cli`）で構成します。

主な機能
- プレビュー生成（露出・ガンマ・sRGB）/ 高品質リサイズ（Lanczos 切替）
- LUT（.cube）適用（1D/3D、プリセット読込・ワンクリック適用）
- ピクセル検査（リニア値、スポイト固定・クリップボードコピー）
- メタデータ閲覧/編集（feature `use_exr_crate`）
- 一括適用（ルール定義 → CLI `apply`）
 - 連番EXRツール（GUI）: FPS一括設定（進捗バー付き・バックアップ作成、成功時に自動削除）、ProRes書き出し（進捗バー）

構成
- `crates/exrtool-core`: 画像ロード/プレビュー/LUT/PNG 書出し、3D LUT 生成、各種ユーティリティ
- `crates/exrtool-cli`: CLI（preview/probe/make-lut1d/make-lut3d/apply/prores※）
- `apps/exrtool-gui`: Tauri GUI（プレビュー、LUTプリセット、PNG保存、Video Tools ほか）

※ `prores` サブコマンドを利用するには `ffmpeg` のインストールが必要です。

セットアップ（Windows 10/11）
- docs/BOOTSTRAP.md の手順に従い、PowerShell（管理者）で実行
  - `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`
  - `./scripts/bootstrap_windows.ps1`

GUI 起動
```bash
cd apps/exrtool-gui/src-tauri
cargo tauri dev
```

注: Video Tools（FPS設定/ProRes）は `exr_pure` 機能を有効にしたGUIビルドが必要です。
例: `cargo tauri dev -- -F exr_pure`

CLI 例（`prores` サブコマンドを使う場合は `ffmpeg` のインストールが必要）
```bash
# プレビューPNGを書き出し（オプション: --lut で .cube 適用、--quality high でHQ）
cargo run -p exrtool-cli -- preview "C:\path\to\input.exr" -o preview.png --max-size 2048 --exposure 0 --gamma 2.2 --quality high

# ピクセル検査
cargo run -p exrtool-cli -- probe "C:\path\to\input.exr" --x 100 --y 200

# 1D LUT（トーン変換）を生成
cargo run -p exrtool-cli -- make-lut1d --src linear --dst srgb --size 1024 -o linear_to_srgb.cube

# 3D LUT（色域+トーン）を生成（33^3、シェーパー1024）
cargo run -p exrtool-cli -- make-lut3d --src-space acescg --src-tf linear --dst-space srgb --dst-tf srgb --size 33 --shaper-size 1024 -o acescg_to_srgb.cube

# ルールに基づく一括適用（PNG書出し）。dry-run/backup対応
cargo run -p exrtool-cli -- apply --rules docs/rules.yml --dry-run false --backup true

# 単一EXRのFPS属性を設定（FramesPerSecond, backupあり）
cargo run -p exrtool-cli -- fps-set --input "C:\\path\\to\\frame.exr" --fps 24 --dry-run false --backup true
```

## Video Tools（GUI）

- Set FPS（連番EXRのFPS属性を書き込み）
  - Videoタブ → Sequence Folder を選択 → FPS/Attribute（既定: `FramesPerSecond`）を指定 → Apply FPS。
  - Dry Run: 対象件数のみ算出（プログレスは即100%）。
  - 実行時: `*.exr.bak` を作成し、安全に書き換え。全件成功時のみバックアップを自動削除。失敗があればバックアップは保持。
  - 進捗: `seq-progress` イベントで0→100%を更新（UIフリーズ防止のためバックグラウンド処理＋スロットリング済み）。
- Export ProRes（EXR連番→ProRes MOV）
  - 依存: `ffmpeg` が PATH 上に必要です。未導入時はエラー表示。
  - Colorspace: `linear:srgb`/`acescg:srgb`/`aces2065:srgb` を選択可能。
  - 進捗: `video-progress` イベントで0→100%を表示。

機能フラグ（features）
- `use_exr_crate`: メタデータ読み書きに `exr` を利用（有効時に `read_metadata`/書出しが動作）
- `use_ocio`（実験的）: OpenColorIO 連携（C FFI）。有効化には OCIO と libclang の開発環境が必要です
  - 例: `cargo build -p exrtool-core --features use_ocio`

### 保存の安全性（EXR書き換え）
- メタデータ書き込みは「一時ファイルに完全書き込み → 置換（Windowsでは既存削除→rename）」方式。
- 失敗時は元ファイルを保持します。`--backup`/GUIのバックアップON時は `*.exr.bak` も残ります。

補足
- LUT プリセットは `config/luts.presets.json` をロードします
- 仕様やアルゴリズムの背景は [docs/LUT.md](docs/LUT.md) を参照
- GUI 操作ガイドは [docs/GUI_GUIDE.md](docs/GUI_GUIDE.md) を参照（スクリーンショットは `docs/img/`）

開発と運用補助
- マージ支援: `scripts/merge_assist.ps1`（PR番号を渡すか、未指定で全オープンPR）
  - 例: `./scripts/merge_assist.ps1 -Numbers 25,26,27`
- ローカルCI: `scripts/ci_local.ps1`（fmt/clippy/build/featureチェック）
