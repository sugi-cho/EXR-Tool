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
    image.write().to_file(path)?;
    Ok(())
}
