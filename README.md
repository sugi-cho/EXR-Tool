# ExrTool (architecture B: Rust core + Tauri)

最小スケルトンです。まずはプレビュー生成とPNG出力、ピクセル検査を実装し、後からOpenImageIO/OCIOのFFIへ差し替え可能な構造にしています。

- `crates/exrtool-core`:
  - EXRロード（将来: OIIO FFI/現状: `exr` crateをfeatureで選択）
  - プレビュー生成（露出・ガンマ・sRGB）
  - LUT（.cube）適用の土台
  - PNG書き出し
- `crates/exrtool-cli`:
  - `preview`: PNGを書き出し
  - `probe`: 指定座標のリニア値を表示
- `apps/exrtool-gui` (Tauri最小):
  - ファイルを開いてプレビュー表示
  - クリック位置のリニア値表示
  - PNG書き出し

現時点ではネットワーク無しのため依存取得・ビルドは未検証です。ビルド時は以下を想定：

```bash
# 例: 特にWindows
rustup toolchain install stable
cargo run -p exrtool-cli -F exr_pure -- preview <in.exr> -o out.png --max-size 2048 --exposure 0.0 --gamma 2.2 [--lut foo.cube]
cargo run -p exrtool-cli -F exr_pure -- probe <in.exr> --x 100 --y 200
```

`exr`クレートの実使用は各パッケージのfeature `exr_pure` または依存の `exrtool-core/use_exr_crate` を有効化した場合に行います（将来はOpenImageIOへ差し替え）。

Tauri GUI の起動（要: Node不要の静的フロントエンド。依存取得が必要です）

```bash
cargo tauri dev -p exrtool-gui -F exr_pure
```
