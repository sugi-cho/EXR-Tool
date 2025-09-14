# TASKS — Codex Cloud 用タスクリスト（優先度付き）

> 本ファイルはクラウド上のCodexで作業するための唯一のタスクソースです。着手前に担当・進捗を明記し、完了時にチェックを入れてください。詳細は AGENTS.md（Cloud Codex 運用）を参照。

## 現在の優先（P0）

1) 即時プレビューの仕上げ（LUT in-memory）
- [x] GUI: Use LUT(in-memory) 既定ON（設定保持は後続）
- [x] update_preview: エラー表示のユーザ通知（UIに赤字で表示）
- [x] 小数点入力の安定化（Exp/Gamma デバウンス）
- [x] クリックでスポイト固定・数値コピー（UI/UX）

2) LUTプリセット（よく使う組合せ）
- [x] GUI: プリセットのドロップダウン（ワンクリック適用）
      - ACEScg(linear) → sRGB(srgb)
      - ACES2065-1(linear) → sRGB(srgb)
      - sRGB(srgb) ↔ Rec.2020(srgb)
- [x] プリセット構成ファイル `config/luts.presets.json` を読み込み
- [x] 「適用のみ」「保存して適用」モード（現状はプレビュー反映＋PNG書き出し）

3) 3D LUT 品質・速度
- [x] コア: 3D LUT 生成を Rayon で並列化（サイズ33/65の高速化）
- [x] オプション: 1Dシェーパー + 3D LUT 出力
- [x] オプション: クリップ切替（Clip/NoClip）
- [x] ベンチマーク: `benches/lut_gen.rs`（17/33/65計測）
- [ ] 生成進捗UI（プログレス/キャンセル）

4) 連番EXR（動画化/メタ情報）
- [x] GUI: FPS一括設定（`FramesPerSecond` 書込み、`exr_pure` 有効時）
  - [x] 進捗バー（`seq-progress`）/ バックグラウンド実行（UIフリーズ防止）
  - [x] バックアップ作成（`*.exr.bak`）/ 全件成功時のみ自動削除
  - [x] 重複属性対策（トップ/レイヤー0集約）/ 安全保存（tmp→置換）
- [x] GUI: 連番EXR→ProRes 書出し（`ffmpeg` 必要、進捗 `video-progress`）
- [ ] CLI: 連番EXRのFPS一括設定（将来公開、GUI 実装をCLIへ移植）
- [ ] CLI: 連番EXR→ProRes 動画化（将来公開）

## 次点（P1）

5) プレビュー品質と操作性
- [x] リサイズ: Lanczos 追加（HQ/標準切替）
- [x] トーンマッピング（ACES/Filmic）
- [x] ヒストグラム（簡易）／ [ ] 波形モニタ（未）

6) メタデータの表示と編集（段階導入）
- [x] コア: `read_metadata(path)`（`use_exr_crate` 有効時）
- [x] 型: Variant 化（基本型／未対応は Opaque）
- [x] GUI: 属性テーブル（閲覧・編集・追加/削除・差分表示）
- [x] 書出: 非破壊保存（別名）

7) 一括編集（最低限）
- [x] ルール定義（YAML/JSON）: set/unset/copy/from filename
- [x] CLI: `exrtool apply --preset rules.yml --dry-run --backup`
- [x] ログ: CSV/JSON 出力（処理記録）

## 将来（P2）

8) OCIO 連携（実験的 / feature `use_ocio`）
- [x] C API バインディング（安全ラッパ層）
- [x] コンフィグ読込・切替（ACES 1.3 等）
- [ ] Display/View/LUTチェインのGUI編集（未）

9) 配布/CI
- [x] GitHub Actions: CLI/GUI のビルド（Win/macOS/Linux）（担当: cloud-codex / 状態: 完了）
- [ ] Windows: MSIX/MSI、macOS: notarize、Linux: AppImage
- [ ] クラッシュ/使用ログ（匿名）オプション

10) 品質・保守
- [x] exrtool-core のユニットテスト（gamma/LUT/マトリクス）
- [x] ドキュメント: `docs/LUT.md`
- [x] `cargo fix`/clippy 警告削減、fmt/lint（継続改善）

11) Video Tools 継続改善
- [ ] FPS設定: キャンセルボタン（進捗イベントと連動）
- [ ] 進捗スロットリング設定のUI化（既定: 100ms/0.5%）
- [ ] エラーレポート集計（成功/失敗件数をダイアログ表示）

---

## 進め方（DoD/受け入れ基準）
- UI/CLI の操作手順が README または docs に追記されている
- エラー時に `log/error.log` に再現手順と全文ログが保存できる
- 1PR=1機能、レビュー観点（目的/実装/検証/影響範囲）が明記されている

## 担当・進捗（例）
- [x] 3D LUT 並列化（担当: cloud-codex / 状態: 完了）
