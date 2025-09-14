# パッケージ署名・検証手順

本プロジェクトで生成される各OS向けパッケージの署名および検証手順をまとめます。

## Windows (MSIX/MSI)
1. `signtool` で署名します。
   ```powershell
   signtool sign /fd SHA256 /a path\to\exrtool.msix
   signtool sign /fd SHA256 /a path\to\exrtool.msi
   ```
2. 署名の検証。
   ```powershell
   signtool verify /pa path\to\exrtool.msix
   signtool verify /pa path\to\exrtool.msi
   ```

## macOS (DMG notarize)
1. `scripts/package_macos.sh` は `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` が設定されている場合、
   `xcrun notarytool` と `xcrun stapler` を使って自動で公証（notarize）を行います。
2. 生成された DMG の検証。
   ```bash
   spctl -a -vv path/to/exrtool.dmg
   ```
   `source=Notarized Developer ID` と表示されれば成功です。

## Linux (AppImage)
1. GPG で署名を生成します。
   ```bash
   gpg --output exrtool.AppImage.sig --detach-sign exrtool.AppImage
   ```
2. 署名の検証。
   ```bash
   gpg --verify exrtool.AppImage.sig exrtool.AppImage
   ```
