# TASKS — Codex Cloud 用タスクリスト（優先度付き）

> 本ファイルはクラウド上のCodexで作業するための唯一のタスクソースです。着手前に担当・進捗を明記し、完了時にチェックを入れてください。詳細は AGENTS.md（Cloud Codex 運用）を参照。

## 現在の優先（P0）

1) 即時プレビューの仕上げ（LUT in-memory）
- [ ] GUI: Use LUT(in-memory) ON時の既定値をONに（設定保持は後続）
- [ ] update_preview: エラー表示のユーザ通知を強化（UIに赤字で表示）
- [ ] 小数点の入力安定化（Exp/GammaでKey入力時のラグ最小化）
- [ ] クリックでスポイト固定・数値コピー（UI/UX）

2) LUTプリセット（よく使う組合せ）
- [ ] GUI: プリセットのドロップダウン（ワンクリック適用）
      - ACEScg(linear) → sRGB(srgb)
      - ACES2065-1(linear) → sRGB(srgb)
      - sRGB(srgb) ↔ Rec.2020(srgb)
- [ ] プリセット構成ファイル（JSON/YAML）を `config/luts.presets.json` に読み込み（将来カスタム可能）
- [ ] 「適用のみ」「保存して適用」両モードを実装

3) 3D LUT 品質・速度
- [ ] コア: 3D LUT 生成を Rayon で並列化（サイズ33/65の高速化）
- [ ] オプション: 1Dシェーパー + 3D LUT 出力（精度/速度の両立）
- [ ] オプション: ソフトクリップ（ロールオフ）とハードクリップの切替
- [ ] ベンチマーク: `benches/lut_gen.rs`（サイズ17/33/65の時間計測）
- [ ] 生成進捗をUI表示（プログレスバー/キャンセル）

## 次点（P1）

4) プレビュー品質と操作性
- [ ] リサイズ: fast_image_resize または Lanczos を追加（HQ/標準を選択）
- [ ] トーンマッピング（ACES/Filmic）を追加、順序（LUT前後）を切替可能に
- [ ] ヒストグラム/波形モニタ（簡易）

5) メタデータの表示と編集（段階導入）
- [ ] コア: `read_metadata(path)` 実装（exr クレートを metadata フィーチャで限定利用）
- [ ] 型: string/int/float/chrono等の基本型をVariant化、未対応はOpaque
- [ ] GUI: 属性テーブル（閲覧、編集、追加/削除、差分ハイライト）
- [ ] 書出: 非破壊保存（別名）→ 後続でアトミック置換対応

6) 一括編集（最低限）
- [ ] ルール定義（YAML/JSON）: set/unset/copy/from filename
- [ ] CLI: `exrtool apply --preset rules.yml --dry-run --backup`
- [ ] ログ: CSV/JSON 出力（ファイル/変更前後/結果）

## 将来（P2）

7) OCIO 連携
- [ ] C API バインディング検討（安全なラッパ層）
- [ ] コンフィグ切替（ACES1.3/社内カスタム）
- [ ] Display/View/LUTチェインをGUIで編集

8) 配布/CI
- [x] GitHub Actions: CLI/GUI のビルド（Win/macOS/Linux）（担当: cloud-codex / 状態: 完了）
- [ ] Windows: MSIX/MSI、macOS: notarize、Linux: AppImage
- [ ] クラッシュ/使用ログ（匿名）オプション

9) 品質・保守
- [ ] exrtool-core のユニットテスト（gamma/LUT/マトリクス）
- [ ] ドキュメント: `docs/LUT.md`（色域／白色点／Bradfordの解説）
- [ ] `cargo fix`/警告ゼロ化、fmt/lint

---

## 進め方（DoD/受け入れ基準）
- UI/CLI の操作手順が README または docs に追記されている
- エラー時に `log/error.log` に再現手順と全文ログが保存できる
- 1PR=1機能、レビュー観点（目的/実装/検証/影響範囲）が明記されている

## 担当・進捗（例）
- [ ] 3D LUT 並列化（担当: cloud-codex / 予定: 0.5d / 状態: 未着手）

