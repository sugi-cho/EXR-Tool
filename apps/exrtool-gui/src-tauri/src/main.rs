#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::fs::{OpenOptions};
use std::io::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use exrtool_core::{export_png, generate_preview, parse_cube, LoadedExr, PreviewImage};

struct AppState {
    image: Option<LoadedExr>,
    preview: Option<PreviewImage>,
    scale: f32, // preview座標→元画像座標への係数 (orig = preview * scale)
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            image: None,
            preview: None,
            scale: 1.0,
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
    log_append(format!("open_exr: path='{}' max={} exp={} gamma={} lut={:?}", path, max_size, exposure, gamma, lut_path).as_str());
    let img = exrtool_core::load_exr_basic(&pathbuf).map_err(|e| { log_append(&format!("open_exr: load failed: {}", e)); e.to_string() })?;
    let lut = if let Some(p) = lut_path {
        let t = std::fs::read_to_string(&p).map_err(|e| { log_append(&format!("open_exr: lut read failed '{}': {}", p, e)); e.to_string() })?;
        Some(parse_cube(&t).map_err(|e| { log_append(&format!("open_exr: lut parse failed: {}", e)); e.to_string() })?)
    } else {
        None
    };
    let preview = generate_preview(&img, max_size, exposure, gamma, lut.as_ref());
    let png = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8.clone())
        .ok_or_else(|| "invalid image".to_string())?;
    let mut buf: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(png)
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageOutputFormat::Png)
        .map_err(|e| e.to_string())?;
    let b64 = BASE64.encode(&buf);

    let scale = (img.width as f32 / preview.width as f32)
        .max(img.height as f32 / preview.height as f32)
        .max(1.0);
    let mut s = state.lock();
    s.image = Some(img);
    s.preview = Some(preview);
    s.scale = scale;
    log_append(&format!("open_exr: ok preview={}x{}", s.preview.as_ref().unwrap().width, s.preview.as_ref().unwrap().height));

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
        _ => { log_append("probe_pixel: image not loaded"); return Err("image not loaded".into()); },
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
fn export_preview_png(state: tauri::State<Arc<Mutex<AppState>>>, out_path: String) -> Result<(), String> {
    let s = state.lock();
    let prev = s
        .preview
        .as_ref()
        .ok_or_else(|| { log_append("export_preview_png: no preview"); "preview not generated".to_string() })?;
    export_png(&PathBuf::from(&out_path), prev).map_err(|e| { log_append(&format!("export_preview_png: failed '{}': {}", out_path, e)); e.to_string() })
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
    // 開発中は src-tauri 直下に固定出力（絶対パス）
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("exrtool-gui.log")
}

fn log_append(msg: &str) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let line = format!("[{}] {}\n", ts, msg);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = f.write_all(line.as_bytes());
    }
}

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(AppState::default())))
        .invoke_handler(tauri::generate_handler![open_exr, probe_pixel, export_preview_png, read_log, clear_log])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
