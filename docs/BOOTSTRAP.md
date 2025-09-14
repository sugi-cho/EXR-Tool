# ExrTool BootStrap / セットアップ手順

このリポジトリを別PCで動かすための最短手順です。

## Windows 10/11

1. PowerShell を管理者で開く（推奨）
2. 実行ポリシーを一時緩和し、スクリプトを実行

```
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
./scripts/bootstrap_windows.ps1
```

3. GUI 開発モードの起動

```
cd apps/exrtool-gui/src-tauri
cargo tauri dev
```

注: Video Tools（FPS設定/ProRes）を使う場合は、EXRメタデータ機能を含むビルドが必要です。

```
cargo tauri dev -- -F exr_pure
```

4. CLI の実行例

```
cargo run -p exrtool-cli -- preview "C:\\path\\to\\input.exr" -o preview.png --max-size 2048 --exposure 0 --gamma 2.2
```

## macOS (参考)

- Homebrew: `brew install rustup-init` → `rustup-init`
- Xcode CLT: `xcode-select --install`
- WebView2 相当は不要（WKWebView）
- tauri-cli: `cargo install tauri-cli --version ^1 --locked`

## Linux (参考)

- 必要ツール: `build-essential libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`
- Rust: `curl https://sh.rustup.rs -sSf | sh`
- tauri-cli: `cargo install tauri-cli --version ^1 --locked`

## よくあるつまずき

- link.exe や cl.exe が無い → VS 2022 Build Tools (C++/Windows SDK) の導入が必要です
- WebView2 runtime が無い → `winget install -e --id Microsoft.EdgeWebView2Runtime`
- `cargo tauri dev -p ...` は不可 → `apps/exrtool-gui/src-tauri` に移動して `cargo tauri dev`
- ProRes 書き出しでエラー → `ffmpeg` を導入し PATH を通してください（Windows 例: `winget install -e --id Gyan.FFmpeg`）

## オプション機能（feature）

- メタデータ（`use_exr_crate`）
  - 有効化例: `cargo build -p exrtool-core --features use_exr_crate`
- OpenColorIO 連携（実験的 `use_ocio`）
  - 前提: OpenColorIO 開発パッケージ、LLVM/Clang（libclang）
  - Windows の場合は vcpkg/conda 等で OCIO を導入し、`LIBCLANG_PATH` を設定
  - 有効化例: `cargo build -p exrtool-core --features use_ocio`
  - feature 無効時は build.rs が自動的にスキップされます

### GUI機能フラグ（apps/exrtool-gui）
- `exr_pure`: 連番EXRのFPS設定（メタデータ書込み）を有効化。
  - 例: `cargo tauri dev -- -F exr_pure`
