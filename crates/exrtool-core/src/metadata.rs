#[cfg(feature = "use_exr_crate")]
use std::collections::HashMap;
#[cfg(feature = "use_exr_crate")]
use std::path::Path;

#[cfg(feature = "use_exr_crate")]
use anyhow::Result;
#[cfg(feature = "use_exr_crate")]
use exr::meta::attribute::{AttributeValue, Text};
#[cfg(feature = "use_exr_crate")]
use exr::prelude::*;

#[cfg(feature = "use_exr_crate")]
use crate::save::save_any_image;

/// Write the provided metadata back to the EXR file.
///
/// If `out` is `Some`, the image is written to that path
/// instead of overwriting the source file.
#[cfg(feature = "use_exr_crate")]
pub fn write_metadata(
    src: &Path,
    metadata: &HashMap<String, String>,
    out: Option<&Path>,
) -> Result<()> {
    let mut image = read_all_data_from_file(src)?;

    for (k, v) in metadata {
        let key = Text::from(k.as_str());
        let val = Text::from(v.as_str());
        let attr = AttributeValue::Text(val.clone());

        // 同名属性の重複を避けるため、まずトップレベルから削除
        image.attributes.other.remove(&key);

        // レイヤー0があればそちらに集約。なければトップレベルに設定。
        if let Some(layer) = image.layer_data.get_mut(0) {
            layer.attributes.other.insert(key, attr);
        } else {
            image.attributes.other.insert(key, AttributeValue::Text(val));
        }
    }

    let target = out.unwrap_or(src);
    save_any_image(&image, target)
}
