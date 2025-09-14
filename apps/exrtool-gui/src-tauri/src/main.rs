#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use exrtool_core::{
    export_png,
    generate_preview,
    load_exr_basic,
    parse_cube,
    LoadedExr,
    Lut,
    PreviewImage,
    PreviewQuality,
    apply_gamma,
    srgb_encode,
    make_3d_lut_cube,
    Primaries,
    TransferFn,
    ClipMode,
};

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
    allow_send: bool, // ログ送信許可
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            image: None,
            preview: None,
            scale: 1.0,
            lut: None,
            allow_send: false,
        }
    }
}

struct OpenProgress {
    cancel: AtomicBool,
}

impl Default for OpenProgress {
    fn default() -> Self {
        Self { cancel: AtomicBool::new(false) }
    }
}

#[derive(Serialize, Deserialize)]
struct AppConfig {
    send_logs: bool,
}

fn config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.json")
}

fn load_config() -> AppConfig {
    match std::fs::read_to_string(config_path()) {
        Ok(t) => serde_json::from_str(&t).unwrap_or(AppConfig { send_logs: false }),
        Err(_) => AppConfig { send_logs: false },
    }
}

fn save_config(cfg: &AppConfig) -> Result<(), String> {
    let s = serde_json::to_string(cfg).map_err(|e| e.to_string())?;
    std::fs::write(config_path(), s).map_err(|e| e.to_string())
}

fn send_async(msg: String) {
    std::thread::spawn(move || {
        log_append(&msg);
    });
}

#[tauri::command]
async fn open_exr(
    window: tauri::Window,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    prog: tauri::State<'_, Arc<OpenProgress>>,
    path: String,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    lut_path: Option<String>,
    high_quality: bool,
) -> Result<(u32, u32, String), String> {
    let pathbuf = PathBuf::from(&path);
    log_append(&format!(
        "open_exr: path='{}' max={} exp={} gamma={} lut={:?} hq={}",
        path, max_size, exposure, gamma, lut_path, high_quality
    ));
    let img = exrtool_core::load_exr_basic(&pathbuf).map_err(|e| {
        log_append(&format!("open_exr: load failed: {}", e));
        e.to_string()
    })?;
    if let Some(ref p) = lut_path {
        match std::fs::read_to_string(&p) {
            Ok(t) => match parse_cube(&t) {
                Ok(lut) => { state.lock().lut = Some(lut); },
                Err(e) => { log_append(&format!("open_exr: lut parse failed: {}", e)); }
            },
            Err(e) => { log_append(&format!("open_exr: lut read failed '{}': {}", p, e)); }
        }
    }

    prog.cancel.store(false, Ordering::SeqCst);
    let s_lut = state.lock().lut.clone();
    if s_lut.is_some() {
        log_append("open_exr: using in-memory LUT");
    } else if lut_path.is_some() {
        log_append("open_exr: using external LUT path");
    } else {
        log_append("open_exr: no LUT");
    }
    let pq = if high_quality { PreviewQuality::High } else { PreviewQuality::Fast };
    let preview = generate_preview(&img, max_size, exposure, gamma, s_lut.as_ref(), pq);
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

    let mut s = state.lock();
    let scale = (img.width as f32 / preview.width as f32)
        .max(img.height as f32 / preview.height as f32)
        .max(1.0);
    s.image = Some(img);
    s.preview = Some(preview);
    s.scale = scale;
    log_append(&format!(
        "open_exr: ok preview={}x{}",
        s.preview.as_ref().unwrap().width,
        s.preview.as_ref().unwrap().height
    ));

    window.emit("open-progress", 100.0).ok();

    Ok((
        s.preview.as_ref().unwrap().width,
        s.preview.as_ref().unwrap().height,
        b64,
    ))
}

#[tauri::command]
fn update_preview(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    lut_path: Option<String>,
    tone_map: Option<String>,
    tone_map_order: Option<String>,
    use_state_lut: bool,
    high_quality: bool,
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
        None => {
            let msg = "update_preview: image not loaded; call open_exr first";
            log_append(msg);
            return Err(msg.into());
        }
    };
    let lut_ref = if use_state_lut { s.lut.as_ref() } else { lut_from_file.as_ref() };
    if use_state_lut {
        log_append(&format!("update_preview: use_state_lut={}, has_state_lut={}", use_state_lut, s.lut.is_some()));
    } else {
        log_append(&format!("update_preview: use_file_lut, file_lut_present={}", lut_from_file.is_some()));
    }
    let pq = if high_quality { PreviewQuality::High } else { PreviewQuality::Fast };
    let preview = generate_preview(img, max_size, exposure, gamma, lut_ref, pq);
    let png = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8.clone())
        .ok_or_else(|| {
            let msg = "update_preview: invalid preview buffer";
            log_append(msg);
            msg.to_string()
        })?;
    let mut buf: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(png)
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageOutputFormat::Png)
        .map_err(|e| {
            let msg = format!("update_preview: encode failed: {}", e);
            log_append(&msg);
            msg
        })?;
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
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
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
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
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

#[tauri::command]
fn cancel_open(prog: tauri::State<'_, Arc<OpenProgress>>) -> Result<(), String> {
    prog.cancel.store(true, Ordering::SeqCst);
    Ok(())
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

fn generate_preview_progress(
    img: &LoadedExr,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    lut: Option<&Lut>,
    window: &tauri::Window,
    prog: &OpenProgress,
) -> Result<PreviewImage, String> {
    let (w, h) = (img.width as u32, img.height as u32);
    let scale = if w <= max_size && h <= max_size {
        1.0
    } else {
        (max_size as f32 / w as f32).min(max_size as f32 / h as f32)
    };
    let out_w = (w as f32 * scale).round().max(1.0) as u32;
    let out_h = (h as f32 * scale).round().max(1.0) as u32;

    let mut rgba8 = vec![0u8; (out_w * out_h * 4) as usize];
    let _ = window.emit("open-progress", 0.0);

    for oy in 0..out_h {
        for ox in 0..out_w {
            let sx = (ox as f32) / scale;
            let sy = (oy as f32) / scale;
            let x0 = sx.floor().clamp(0.0, (w - 1) as f32) as i32;
            let y0 = sy.floor().clamp(0.0, (h - 1) as f32) as i32;
            let x1 = (x0 + 1).min(w as i32 - 1);
            let y1 = (y0 + 1).min(h as i32 - 1);
            let tx = (sx - x0 as f32).clamp(0.0, 1.0);
            let ty = (sy - y0 as f32).clamp(0.0, 1.0);

            let sample = |x:i32,y:i32| -> (f32,f32,f32,f32) {
                let idx = (y as usize * img.width + x as usize) * 4;
                (
                    img.rgba_f32[idx+0],
                    img.rgba_f32[idx+1],
                    img.rgba_f32[idx+2],
                    img.rgba_f32[idx+3]
                )
            };
            let (r00,g00,b00,a00) = sample(x0,y0);
            let (r10,g10,b10,a10) = sample(x1,y0);
            let (r01,g01,b01,a01) = sample(x0,y1);
            let (r11,g11,b11,a11) = sample(x1,y1);
            let lerp = |a:f32,b:f32,t:f32| a + (b-a)*t;
            let r0 = lerp(r00,r10,tx); let r1 = lerp(r01,r11,tx); let mut r = lerp(r0,r1,ty);
            let g0 = lerp(g00,g10,tx); let g1 = lerp(g01,g11,tx); let mut g = lerp(g0,g1,ty);
            let b0 = lerp(b00,b10,tx); let b1 = lerp(b01,b11,tx); let mut b = lerp(b0,b1,ty);
            let a0 = lerp(a00,a10,tx); let a1 = lerp(a01,a11,tx); let a = lerp(a0,a1,ty);

            let m = 2.0f32.powf(exposure);
            r *= m; g *= m; b *= m;

            if let Some(l) = lut {
                let rgb = l.apply([r, g, b]);
                r = rgb[0]; g = rgb[1]; b = rgb[2];
            }

            let rgb = apply_gamma([r, g, b], gamma);
            let (r8, g8, b8) = (srgb_encode(rgb[0]), srgb_encode(rgb[1]), srgb_encode(rgb[2]));

            let di = (oy * out_w + ox) as usize * 4;
            rgba8[di + 0] = r8;
            rgba8[di + 1] = g8;
            rgba8[di + 2] = b8;
            rgba8[di + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        let pct = ((oy + 1) as f64 / out_h as f64) * 100.0;
        let _ = window.emit("open-progress", pct);
        if prog.cancel.load(Ordering::SeqCst) {
            return Err("cancelled".to_string());
        }
    }

    Ok(PreviewImage { width: out_w, height: out_h, rgba8 })
}

#[tauri::command]
fn set_lut_1d(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
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
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    src_space: String,
    src_tf: String,
    dst_space: String,
    dst_tf: String,
    size: u32,
    clip_mode: String,
) -> Result<(), String> {
    use exrtool_core::{make_3d_lut_cube, Primaries, TransferFn, ClipMode};
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
    let parse_clip = |s: &str| -> Result<ClipMode, String> {
        match s.to_ascii_lowercase().as_str() {
            "clip" => Ok(ClipMode::Clip),
            "noclip" | "none" => Ok(ClipMode::NoClip),
            _ => Err(format!("unknown clip mode: {}", s)),
        }
    };
    let text = make_3d_lut_cube(
        parse_space(&src_space)?,
        parse_tf(&src_tf)?,
        parse_space(&dst_space)?,
        parse_tf(&dst_tf)?,
        size as usize,
        1024,
    );
    let lut = parse_cube(&text).map_err(|e| e.to_string())?;
    state.lock().lut = Some(lut);
    Ok(())
}

#[tauri::command]
fn clear_lut(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<(), String> {
    state.lock().lut = None;
    Ok(())
}

#[tauri::command]
fn read_metadata(path: String) -> Result<Vec<(String, String)>, String> {
    let p = std::path::Path::new(&path);
    // try core's read_metadata (works when feature `use_exr_crate` is enabled)
    match exrtool_core::read_metadata(p) {
        Ok(meta) => {
            // Flatten headers into key-value pairs for simple display
            let mut out: Vec<(String, String)> = Vec::new();
            for (i, h) in meta.headers.iter().enumerate() {
                out.push((format!("header{}.layer_name", i), h.layer_name.clone().unwrap_or_default()));
                out.push((format!("header{}.layer_pos", i), format!("{},{}", h.layer_position.0, h.layer_position.1)));
                out.push((format!("header{}.layer_size", i), format!("{}x{}", h.layer_size.0, h.layer_size.1)));
                out.push((format!("header{}.pixel_aspect", i), format!("{}", h.pixel_aspect)));
                out.push((format!("header{}.line_order", i), h.line_order.clone()));
            }
            Ok(out)
        }
        Err(_e) => {
            // Feature未有効など。空で返却（フロントはログに記録）
            Ok(Vec::new())
        }
    }
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
        1024,
    );
    std::fs::write(out_path, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn lut_presets(state: tauri::State<'_, PresetState>) -> Result<Vec<LutPreset>, String> {
    Ok(state.presets.clone())
}

#[tauri::command]
fn get_log_permission(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<bool, String> {
    Ok(state.lock().allow_send)
}

#[tauri::command]
fn set_log_permission(
    state: tauri::State<Arc<Mutex<AppState>>>,
    allow: bool,
) -> Result<(), String> {
    {
        let mut s = state.lock();
        s.allow_send = allow;
    }
    save_config(&AppConfig { send_logs: allow })?;
    Ok(())
}

#[tauri::command]
fn write_log(s: String) -> Result<(), String> {
    log_append(&s);
    Ok(())
}

// --- Video / Sequence commands ---
#[derive(Serialize)]
struct SeqSummary {
    success: usize,
    failure: usize,
}
#[tauri::command]
async fn seq_fps(
    window: tauri::Window,
    dir: String,
    fps: f32,
    attr: Option<String>,
    recursive: bool,
    dry_run: bool,
    backup: bool,
) -> Result<SeqSummary, String> {
    #[cfg(feature = "exr_pure")]
    {
        use std::{collections::HashMap, path::PathBuf};
        let window_clone = window.clone();
        let result = tauri::async_runtime::spawn_blocking(move || -> Result<SeqSummary, String> {
            fn collect(dir: &PathBuf, recursive: bool, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
                for entry in std::fs::read_dir(dir)? {
                    let e = entry?; let p = e.path();
                    if p.is_dir() { if recursive { collect(&p, recursive, out)?; } }
                    else if p.extension().map(|s| s.to_string_lossy().to_ascii_lowercase()) == Some("exr".into()) { out.push(p); }
                }
                Ok(())
            }
            use std::time::{Duration, Instant};
            let mut files = Vec::new();
            let d = PathBuf::from(dir);
            collect(&d, recursive, &mut files).map_err(|e| e.to_string())?;
            files.sort_by(|a,b| a.file_name().unwrap().cmp(b.file_name().unwrap()));
            let total_files = files.len();
            let total = total_files.max(1) as f64;
            let _ = window_clone.emit("seq-progress", 0.0);
            if dry_run {
                let _ = window_clone.emit("seq-progress", 100.0);
                return Ok(SeqSummary { success: total_files, failure: 0 });
            }
            let mut map = HashMap::new();
            map.insert(attr.unwrap_or_else(|| "FramesPerSecond".into()), format!("{}", fps));
            let mut ok = 0usize;
            let mut baks: Vec<PathBuf> = Vec::new();
            let mut last_emit = Instant::now();
            let mut last_pct: f64 = -1.0;
            for (i, f) in files.iter().enumerate() {
                if backup && !dry_run {
                    let bak = f.with_extension("exr.bak");
                    if let Err(e) = std::fs::copy(&f, &bak) {
                        log_append(&format!("seq_fps backup failed {} -> {}: {}", f.display(), bak.display(), e));
                    } else { baks.push(bak); }
                }
                match exrtool_core::metadata::write_metadata(&f, &map, None) {
                    Ok(_) => ok += 1,
                    Err(e) => log_append(&format!("seq_fps failed {}: {}", f.display(), e)),
                }
                let pct = (((i as f64) + 1.0) / total * 100.0) as f64;
                if pct - last_pct >= 0.5 || last_emit.elapsed() >= Duration::from_millis(100) || (i + 1) == files.len() {
                    let _ = window_clone.emit("seq-progress", pct);
                    last_pct = pct;
                    last_emit = Instant::now();
                }
            }
            if ok as f64 == total {
                for b in baks { let _ = std::fs::remove_file(&b); }
            } else {
                log_append("seq_fps: errors occurred; backups are kept");
            }
            Ok(SeqSummary { success: ok, failure: total_files.saturating_sub(ok) })
        }).await.map_err(|e| e.to_string())?;
        result
    }
    #[cfg(not(feature = "exr_pure"))]
    {
        Err("This build does not include EXR metadata support. Rebuild with feature exr_pure.".into())
    }
}

#[tauri::command]
fn export_prores(
    dir: String,
    fps: f32,
    colorspace: String,
    out: String,
    profile: String,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    quality: String,
    window: tauri::Window,
) -> Result<(), String> {
    use std::process::{Command, Stdio};
    if Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() {
        return Err("ffmpeg not found. Please install ffmpeg and ensure it's on PATH.".into());
    }
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?; let p = e.path();
        if p.is_file() && p.extension().map(|s| s.to_string_lossy().to_ascii_lowercase())==Some("exr".into()) { files.push(p); }
    }
    files.sort_by(|a,b| a.file_name().unwrap().cmp(b.file_name().unwrap()));
    if files.is_empty() { return Err("no EXR files found".into()); }
    // LUT
    let mut lut_obj = None;
    let cs = colorspace.to_lowercase();
    if cs != "linear:srgb" {
        use exrtool_core::{make_3d_lut_cube, Primaries, TransferFn};
        let (sp, dp) = if cs=="acescg:srgb" { (Primaries::ACEScgD60, Primaries::SrgbD65) } else if cs=="aces2065:srgb" { (Primaries::ACES2065_1D60, Primaries::SrgbD65) } else { (Primaries::SrgbD65, Primaries::SrgbD65) };
        let text = make_3d_lut_cube(sp, TransferFn::Linear, dp, TransferFn::Srgb, 33, 1024);
        lut_obj = Some(parse_cube(&text).map_err(|e| e.to_string())?);
    }
    // spawn ffmpeg
    let mut child = Command::new("ffmpeg")
        .arg("-y").arg("-f").arg("image2pipe").arg("-r").arg(format!("{}", fps))
        .arg("-vcodec").arg("png").arg("-i").arg("-")
        .arg("-c:v").arg("prores_ks").arg("-profile:v").arg(match profile.as_str() { "422hq"=>"3", "422"=>"2", "4444"=>"4", _=>"3" })
        .arg(out)
        .stdin(Stdio::piped())
        .spawn().map_err(|e| e.to_string())?;
    {
        use image::ImageOutputFormat;
        use std::io::Write;
        let mut stdin = child.stdin.take().ok_or("failed to open ffmpeg stdin")?;
        let total = files.len() as f64;
        for (i, f) in files.iter().enumerate() {
            let img = load_exr_basic(f).map_err(|e| e.to_string())?;
            let pq = if quality.to_lowercase()=="high" { PreviewQuality::High } else { PreviewQuality::Fast };
            let preview = generate_preview(&img, max_size, exposure, gamma, lut_obj.as_ref(), pq);
            let buf = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8).ok_or("invalid buffer")?;
            let mut bytes: Vec<u8> = Vec::new();
            image::DynamicImage::ImageRgba8(buf).write_to(&mut std::io::Cursor::new(&mut bytes), ImageOutputFormat::Png).map_err(|e| e.to_string())?;
            stdin.write_all(&bytes).map_err(|e| e.to_string())?;
            let _ = window.emit("video-progress", ((i as f64 + 1.0)/total*100.0) as f64);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() { return Err(format!("ffmpeg exited with {:?}", status)); }
    Ok(())
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
        .manage(Arc::new(OpenProgress::default()))
        .manage(PresetState { presets })
        .invoke_handler(tauri::generate_handler![
            open_exr,
            update_preview,
            probe_pixel,
            export_preview_png,
            read_log,
            clear_log,
            cancel_open,
            set_lut_1d,
            set_lut_3d,
            clear_lut,
            lut_presets,
            read_metadata,
            make_lut,
            make_lut3d,
            seq_fps,
            export_prores,
            write_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
