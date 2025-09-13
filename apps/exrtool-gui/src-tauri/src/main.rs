#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use exrtool_core::{export_png, generate_preview, parse_cube, LoadedExr, Lut, PreviewImage};

#[derive(Clone, Serialize, Deserialize)]
struct LutPreset {
    name: String,
    src_space: String,
    src_tf: String,
    dst_space: String,
    dst_tf: String,
    size: u32,
}

struct PresetState {
    presets: Vec<LutPreset>,
}

struct AppState {
    image: Option<LoadedExr>,
    preview: Option<PreviewImage>,
    scale: f32,       // preview座標→元画像座標への係数 (orig = preview * scale)
    lut: Option<Lut>, // メモリ内LUT（即時プレビュー用）
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            image: None,
            preview: None,
            scale: 1.0,
            lut: None,
        }
    }
}

#[tauri::command]
fn open_exr(
    state: tauri::State<Arc<Mutex<AppState>>>,
    path: String,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    lut_path: Option<String>,
) -> Result<(u32, u32, String), String> {
    let pathbuf = PathBuf::from(&path);
    log_append(&format!(
        "open_exr: path='{}' max={} exp={} gamma={} lut={:?}",
        path, max_size, exposure, gamma, lut_path
    ));
    let img = exrtool_core::load_exr_basic(&pathbuf).map_err(|e| {
        log_append(&format!("open_exr: load failed: {}", e));
        e.to_string()
    })?;
    if let Some(p) = lut_path {
        match std::fs::read_to_string(&p) {
            Ok(t) => match parse_cube(&t) {
                Ok(lut) => state.lock().lut = Some(lut),
                Err(e) => log_append(&format!("open_exr: lut parse failed: {}", e)),
            },
            Err(e) => log_append(&format!("open_exr: lut read failed '{}': {}", p, e)),
        }
    }

    let s_lut = state.lock().lut.clone();
    let preview = generate_preview(&img, max_size, exposure, gamma, s_lut.as_ref());
    let png = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8.clone())
        .ok_or_else(|| "invalid image".to_string())?;
    let mut buf: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(png)
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageOutputFormat::Png,
        )
        .map_err(|e| e.to_string())?;
    let b64 = BASE64.encode(&buf);

    let scale = (img.width as f32 / preview.width as f32)
        .max(img.height as f32 / preview.height as f32)
        .max(1.0);
    let mut s = state.lock();
    s.image = Some(img);
    s.preview = Some(preview);
    s.scale = scale;
    log_append(&format!(
        "open_exr: ok preview={}x{}",
        s.preview.as_ref().unwrap().width,
        s.preview.as_ref().unwrap().height
    ));

    Ok((
        s.preview.as_ref().unwrap().width,
        s.preview.as_ref().unwrap().height,
        b64,
    ))
}

#[tauri::command]
fn update_preview(
    state: tauri::State<Arc<Mutex<AppState>>>,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    lut_path: Option<String>,
    use_state_lut: bool,
) -> Result<(u32, u32, String), String> {
    // 事前にファイルからLUTを読み込んでおく（必要なら）
    let lut_from_file: Option<Lut> = if !use_state_lut {
        if let Some(p) = &lut_path {
            match std::fs::read_to_string(p) {
                Ok(t) => match parse_cube(&t) {
                    Ok(l) => Some(l),
                    Err(e) => {
                        log_append(&format!("update_preview: lut parse failed: {}", e));
                        None
                    }
                },
                Err(e) => {
                    log_append(&format!("update_preview: lut read failed '{}': {}", p, e));
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let mut s = state.lock();
    let img = match s.image.as_ref() {
        Some(img) => img,
        None => return Err("image not loaded".into()),
    };
    let lut_ref = if use_state_lut {
        s.lut.as_ref()
    } else {
        lut_from_file.as_ref()
    };
    let preview = generate_preview(img, max_size, exposure, gamma, lut_ref);
    let png = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8.clone())
        .ok_or_else(|| "invalid image".to_string())?;
    let mut buf: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(png)
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageOutputFormat::Png,
        )
        .map_err(|e| e.to_string())?;
    let b64 = BASE64.encode(&buf);

    s.scale = (img.width as f32 / preview.width as f32)
        .max(img.height as f32 / preview.height as f32)
        .max(1.0);
    s.preview = Some(preview);
    if !use_state_lut {
        if let Some(l) = lut_from_file {
            s.lut = Some(l);
        }
    }
    Ok((
        s.preview.as_ref().unwrap().width,
        s.preview.as_ref().unwrap().height,
        b64,
    ))
}

#[tauri::command]
fn probe_pixel(
    state: tauri::State<Arc<Mutex<AppState>>>,
    px: u32,
    py: u32,
) -> Result<(f32, f32, f32, f32), String> {
    let s = state.lock();
    let (img, scale) = match (&s.image, s.scale) {
        (Some(img), sc) => (img, sc),
        _ => {
            log_append("probe_pixel: image not loaded");
            return Err("image not loaded".into());
        }
    };
    let ox = ((px as f32) * scale).floor() as usize;
    let oy = ((py as f32) * scale).floor() as usize;
    let p = img.get_linear(ox, oy).ok_or_else(|| {
        log_append(&format!("probe_pixel: out of range ({},{})", ox, oy));
        "coordinate out of range".to_string()
    })?;
    Ok((p.r, p.g, p.b, p.a))
}

#[tauri::command]
fn export_preview_png(
    state: tauri::State<Arc<Mutex<AppState>>>,
    out_path: String,
) -> Result<(), String> {
    let s = state.lock();
    let prev = s.preview.as_ref().ok_or_else(|| {
        log_append("export_preview_png: no preview");
        "preview not generated".to_string()
    })?;
    export_png(&PathBuf::from(&out_path), prev).map_err(|e| {
        log_append(&format!("export_preview_png: failed '{}': {}", out_path, e));
        e.to_string()
    })
}

#[tauri::command]
fn read_log() -> Result<String, String> {
    match std::fs::read_to_string(log_path()) {
        Ok(s) => Ok(s),
        Err(e) => Ok(format!("<no log: {}>", e)),
    }
}

#[tauri::command]
fn clear_log() -> Result<(), String> {
    std::fs::write(log_path(), "").map_err(|e| e.to_string())
}

fn log_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("exrtool-gui.log")
}

fn log_append(msg: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let line = format!("[{}] {}\n", ts, msg);
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = f.write_all(line.as_bytes());
    }
}

#[tauri::command]
fn set_lut_1d(
    state: tauri::State<Arc<Mutex<AppState>>>,
    src: String,
    dst: String,
    size: u32,
) -> Result<(), String> {
    use exrtool_core::{make_1d_lut, ColorSpace};
    let parse = |s: &str| -> Result<ColorSpace, String> {
        match s.to_ascii_lowercase().as_str() {
            "linear" => Ok(ColorSpace::Linear),
            "srgb" => Ok(ColorSpace::Srgb),
            _ => Err(format!("unknown colorspace: {}", s)),
        }
    };
    let text = make_1d_lut(parse(&src)?, parse(&dst)?, size as usize);
    let lut = parse_cube(&text).map_err(|e| e.to_string())?;
    state.lock().lut = Some(lut);
    Ok(())
}

#[tauri::command]
fn set_lut_3d(
    state: tauri::State<Arc<Mutex<AppState>>>,
    src_space: String,
    src_tf: String,
    dst_space: String,
    dst_tf: String,
    size: u32,
) -> Result<(), String> {
    use exrtool_core::{make_3d_lut_cube, Primaries, TransferFn};
    let parse_space = |s: &str| -> Result<Primaries, String> {
        match s.to_ascii_lowercase().as_str() {
            "srgb" | "rec709" => Ok(Primaries::SrgbD65),
            "rec2020" | "bt2020" => Ok(Primaries::Rec2020D65),
            "acescg" | "ap1" => Ok(Primaries::ACEScgD60),
            "aces2065" | "ap0" | "aces" => Ok(Primaries::ACES2065_1D60),
            _ => Err(format!("unknown space: {}", s)),
        }
    };
    let parse_tf = |s: &str| -> Result<TransferFn, String> {
        match s.to_ascii_lowercase().as_str() {
            "linear" => Ok(TransferFn::Linear),
            "srgb" => Ok(TransferFn::Srgb),
            "g24" | "gamma2.4" => Ok(TransferFn::Gamma24),
            "g22" | "gamma2.2" => Ok(TransferFn::Gamma22),
            _ => Err(format!("unknown transfer: {}", s)),
        }
    };
    let text = make_3d_lut_cube(
        parse_space(&src_space)?,
        parse_tf(&src_tf)?,
        parse_space(&dst_space)?,
        parse_tf(&dst_tf)?,
        size as usize,
    );
    let lut = parse_cube(&text).map_err(|e| e.to_string())?;
    state.lock().lut = Some(lut);
    Ok(())
}

#[tauri::command]
fn clear_lut(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<(), String> {
    state.lock().lut = None;
    Ok(())
}

#[tauri::command]
fn make_lut(src: String, dst: String, size: u32, out_path: String) -> Result<(), String> {
    use exrtool_core::{make_1d_lut, ColorSpace};
    let parse = |s: &str| -> Result<ColorSpace, String> {
        match s.to_ascii_lowercase().as_str() {
            "linear" => Ok(ColorSpace::Linear),
            "srgb" => Ok(ColorSpace::Srgb),
            _ => Err(format!("unknown colorspace: {}", s)),
        }
    };
    let text = make_1d_lut(parse(&src)?, parse(&dst)?, size as usize);
    std::fs::write(out_path, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn make_lut3d(
    src_space: String,
    src_tf: String,
    dst_space: String,
    dst_tf: String,
    size: u32,
    out_path: String,
) -> Result<(), String> {
    use exrtool_core::{make_3d_lut_cube, Primaries, TransferFn};
    let parse_space = |s: &str| -> Result<Primaries, String> {
        match s.to_ascii_lowercase().as_str() {
            "srgb" | "rec709" => Ok(Primaries::SrgbD65),
            "rec2020" | "bt2020" => Ok(Primaries::Rec2020D65),
            "acescg" | "ap1" => Ok(Primaries::ACEScgD60),
            "aces2065" | "ap0" | "aces" => Ok(Primaries::ACES2065_1D60),
            _ => Err(format!("unknown space: {}", s)),
        }
    };
    let parse_tf = |s: &str| -> Result<TransferFn, String> {
        match s.to_ascii_lowercase().as_str() {
            "linear" => Ok(TransferFn::Linear),
            "srgb" => Ok(TransferFn::Srgb),
            "g24" | "gamma2.4" => Ok(TransferFn::Gamma24),
            "g22" | "gamma2.2" => Ok(TransferFn::Gamma22),
            _ => Err(format!("unknown transfer: {}", s)),
        }
    };
    let text = make_3d_lut_cube(
        parse_space(&src_space)?,
        parse_tf(&src_tf)?,
        parse_space(&dst_space)?,
        parse_tf(&dst_tf)?,
        size as usize,
    );
    std::fs::write(out_path, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn lut_presets(state: tauri::State<PresetState>) -> Result<Vec<LutPreset>, String> {
    Ok(state.presets.clone())
}

fn main() {
    let preset_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config/luts.presets.json");
    let presets: Vec<LutPreset> = std::fs::read_to_string(&preset_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();

    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(AppState::default())))
        .manage(PresetState { presets })
        .invoke_handler(tauri::generate_handler![
            open_exr,
            update_preview,
            probe_pixel,
            export_preview_png,
            read_log,
            clear_log,
            set_lut_1d,
            set_lut_3d,
            clear_lut,
            lut_presets
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
