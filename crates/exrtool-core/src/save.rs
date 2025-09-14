#[cfg(feature = "use_exr_crate")]
use anyhow::Result;
#[cfg(feature = "use_exr_crate")]
use exr::prelude::*;
#[cfg(feature = "use_exr_crate")]
use std::path::Path;

/// Save an EXR image to the specified path.
///
/// This wrapper exists so the write mechanism can later
/// be replaced with an atomic operation.
#[cfg(feature = "use_exr_crate")]
pub fn save_any_image(image: &AnyImage, path: &Path) -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // 一時ファイルへ書き込み後に置き換え（疑似アトミック）
    let parent = path.parent().ok_or_else(|| anyhow::anyhow!("invalid path"))?;
    let stem = path.file_name().ok_or_else(|| anyhow::anyhow!("invalid file name"))?.to_string_lossy();
    let tmp: PathBuf = parent.join(format!("._{}_tmp_{}", stem, std::process::id()));

    // 1) まず一時ファイルへ完全書き込み
    image.write().to_file(&tmp)?;

    // 2) 置き換え（Windowsでrename上書き不可の場合は削除→rename）
    match fs::rename(&tmp, path) {
        Ok(_) => Ok(()),
        Err(_) => {
            // 既存ファイルを削除してから差し替え
            let _ = fs::remove_file(path);
            fs::rename(&tmp, path)?;
            Ok(())
        }
    }
}
