#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/../apps/exrtool-gui/src-tauri"

cargo tauri build --bundles appimage
