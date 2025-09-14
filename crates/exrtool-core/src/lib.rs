pub mod rules;
use anyhow::{anyhow, Result};
use image::imageops::FilterType;
use nalgebra::{Matrix3, Vector3};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(feature = "use_exr_crate")]
pub mod metadata;
#[cfg(feature = "use_ocio")]
pub mod ocio;
#[cfg(feature = "use_exr_crate")]
mod save;

// Minimal metadata structures used by read_metadata() regardless of feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExrHeaderData {
    pub layer_name: Option<String>,
    pub layer_position: (i32, i32),
    pub layer_size: (u32, u32),
    pub pixel_aspect: f32,
    pub line_order: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExrMetadata {
    pub headers: Vec<ExrHeaderData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    // sRGB 8-bit RGBA
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearPixel {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ToneMapKind {
    None,
    Aces,
    Filmic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ToneMapOrder {
    BeforeLut,
    AfterLut,
}

#[derive(Debug)]
pub struct LoadedExr {
    pub width: usize,
    pub height: usize,
    // interleaved RGBA (linear, f32; a=1.0 if absent)
    pub rgba_f32: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageStats {
    pub hist_r: Vec<u32>,
    pub hist_g: Vec<u32>,
    pub hist_b: Vec<u32>,
}

/// Compute per-channel histogram (0..255) from preview image.
pub fn compute_image_stats(preview: &PreviewImage, bins: usize) -> ImageStats {
    let mut hist_r = vec![0u32; bins];
    let mut hist_g = vec![0u32; bins];
    let mut hist_b = vec![0u32; bins];
    let scale = (bins.saturating_sub(1)) as f32 / 255.0;
    for px in preview.rgba8.chunks_exact(4) {
        let r = (px[0] as f32 * scale).round() as usize;
        let g = (px[1] as f32 * scale).round() as usize;
        let b = (px[2] as f32 * scale).round() as usize;
        hist_r[r.min(bins - 1)] += 1;
        hist_g[g.min(bins - 1)] += 1;
        hist_b[b.min(bins - 1)] += 1;
    }
    ImageStats {
        hist_r,
        hist_g,
        hist_b,
    }
}

impl LoadedExr {
    pub fn get_linear(&self, x: usize, y: usize) -> Option<LinearPixel> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y * self.width + x) * 4;
        Some(LinearPixel {
            r: self.rgba_f32[idx + 0],
            g: self.rgba_f32[idx + 1],
            b: self.rgba_f32[idx + 2],
            a: self.rgba_f32[idx + 3],
        })
    }
}

// ---- EXR Metadata (via exr crate) ----
#[cfg(feature = "use_exr_crate")]
pub fn read_metadata(path: &Path) -> Result<ExrMetadata> {
    use exr::meta::MetaData;
    let meta = MetaData::read_from_file(path, false)?;
    let headers = meta
        .headers
        .into_iter()
        .map(|h| ExrHeaderData {
            layer_name: h.own_attributes.layer_name.map(|t| t.to_string()),
            layer_position: (
                h.own_attributes.layer_position.0,
                h.own_attributes.layer_position.1,
            ),
            layer_size: (h.layer_size.0 as u32, h.layer_size.1 as u32),
            pixel_aspect: h.shared_attributes.pixel_aspect,
            line_order: format!("{:?}", h.line_order),
        })
        .collect();
    Ok(ExrMetadata { headers })
}

#[cfg(not(feature = "use_exr_crate"))]
pub fn read_metadata(_path: &Path) -> Result<ExrMetadata> {
    Err(anyhow!("feature `use_exr_crate` is not enabled"))
}

// ---- EXR Loading (via image crate) ----
pub fn load_exr_basic(path: &Path) -> Result<LoadedExr> {
    // Use image crate EXR decoder (feature = "exr").
    let dynimg = image::open(path)?; // DynamicImage
    let rgba = dynimg.to_rgba32f(); // ImageBuffer<Rgba<f32>, Vec<f32>>
    let (w, h) = rgba.dimensions();
    let data = rgba.into_raw(); // Vec<f32> length = w*h*4
    if data.len() != (w as usize * h as usize * 4) {
        return Err(anyhow!("invalid rgba32f buffer size"));
    }
    Ok(LoadedExr {
        width: w as usize,
        height: h as usize,
        rgba_f32: data,
    })
}

// ---- Preview Generation ----
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PreviewQuality {
    Fast,
    High,
}

pub fn generate_preview(
    img: &LoadedExr,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    lut: Option<&Lut>,
    quality: PreviewQuality,
) -> PreviewImage {
    let (w, h) = (img.width as u32, img.height as u32);
    let scale = if w <= max_size && h <= max_size {
        1.0
    } else {
        (max_size as f32 / w as f32).min(max_size as f32 / h as f32)
    };
    let out_w = (w as f32 * scale).round().max(1.0) as u32;
    let out_h = (h as f32 * scale).round().max(1.0) as u32;

    let mut rgba8 = vec![0u8; (out_w * out_h * 4) as usize];

    match quality {
        PreviewQuality::Fast => {
            for oy in 0..out_h {
                for ox in 0..out_w {
                    // bilinear sampling
                    let sx = (ox as f32) / scale;
                    let sy = (oy as f32) / scale;
                    let x0 = sx.floor().clamp(0.0, (w - 1) as f32) as i32;
                    let y0 = sy.floor().clamp(0.0, (h - 1) as f32) as i32;
                    let x1 = (x0 + 1).min(w as i32 - 1);
                    let y1 = (y0 + 1).min(h as i32 - 1);
                    let tx = (sx - x0 as f32).clamp(0.0, 1.0);
                    let ty = (sy - y0 as f32).clamp(0.0, 1.0);

                    let sample = |x: i32, y: i32| -> (f32, f32, f32, f32) {
                        let idx = (y as usize * img.width + x as usize) * 4;
                        (
                            img.rgba_f32[idx + 0],
                            img.rgba_f32[idx + 1],
                            img.rgba_f32[idx + 2],
                            img.rgba_f32[idx + 3],
                        )
                    };
                    let (r00, g00, b00, a00) = sample(x0, y0);
                    let (r10, g10, b10, a10) = sample(x1, y0);
                    let (r01, g01, b01, a01) = sample(x0, y1);
                    let (r11, g11, b11, a11) = sample(x1, y1);
                    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
                    let r0 = lerp(r00, r10, tx);
                    let r1 = lerp(r01, r11, tx);
                    let mut r = lerp(r0, r1, ty);
                    let g0 = lerp(g00, g10, tx);
                    let g1 = lerp(g01, g11, tx);
                    let mut g = lerp(g0, g1, ty);
                    let b0 = lerp(b00, b10, tx);
                    let b1 = lerp(b01, b11, tx);
                    let mut b = lerp(b0, b1, ty);
                    let a0 = lerp(a00, a10, tx);
                    let a1 = lerp(a01, a11, tx);
                    let a = lerp(a0, a1, ty);

                    // exposure in stops (2^exposure)
                    let m = 2.0f32.powf(exposure);
                    r *= m;
                    g *= m;
                    b *= m;

                    if let Some(l) = lut {
                        let rgb = l.apply([r, g, b]);
                        r = rgb[0];
                        g = rgb[1];
                        b = rgb[2];
                    }

                    let rgb = apply_gamma([r, g, b], gamma);
                    let (r8, g8, b8) = (
                        srgb_encode(rgb[0]),
                        srgb_encode(rgb[1]),
                        srgb_encode(rgb[2]),
                    );

                    let di = (oy * out_w + ox) as usize * 4;
                    rgba8[di + 0] = r8;
                    rgba8[di + 1] = g8;
                    rgba8[di + 2] = b8;
                    rgba8[di + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
        }
        PreviewQuality::High => {
            let src = image::ImageBuffer::<image::Rgba<f32>, Vec<f32>>::from_raw(
                w,
                h,
                img.rgba_f32.clone(),
            )
            .expect("invalid rgba buffer");
            let resized = image::imageops::resize(&src, out_w, out_h, FilterType::Lanczos3);
            let data = resized.into_vec();
            for oy in 0..out_h {
                for ox in 0..out_w {
                    let idx = (oy * out_w + ox) as usize * 4;
                    let mut r = data[idx + 0];
                    let mut g = data[idx + 1];
                    let mut b = data[idx + 2];
                    let a = data[idx + 3];

                    let m = 2.0f32.powf(exposure);
                    r *= m;
                    g *= m;
                    b *= m;

                    if let Some(l) = lut {
                        let rgb = l.apply([r, g, b]);
                        r = rgb[0];
                        g = rgb[1];
                        b = rgb[2];
                    }

                    let rgb = apply_gamma([r, g, b], gamma);
                    let (r8, g8, b8) = (
                        srgb_encode(rgb[0]),
                        srgb_encode(rgb[1]),
                        srgb_encode(rgb[2]),
                    );

                    rgba8[idx + 0] = r8;
                    rgba8[idx + 1] = g8;
                    rgba8[idx + 2] = b8;
                    rgba8[idx + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
        }
    }

    PreviewImage {
        width: out_w,
        height: out_h,
        rgba8,
    }
}

pub fn export_png(path: &Path, preview: &PreviewImage) -> Result<()> {
    let img = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8.clone())
        .ok_or_else(|| anyhow!("failed to create image buffer"))?;
    image::DynamicImage::ImageRgba8(img).save(path)?;
    Ok(())
}

// ---- LUT (.cube minimal) ----
#[derive(Debug, Clone)]
pub struct Lut {
    shaper_size: usize,
    shaper_table: Vec<[f32; 3]>,
    cube_size: usize,
    cube_table: Vec<[f32; 3]>,
}

impl Lut {
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let mut v = rgb;
        if self.shaper_size > 0 {
            v = apply_1d(&v, self.shaper_size, &self.shaper_table);
        }
        if self.cube_size > 0 {
            v = apply_3d(&v, self.cube_size, &self.cube_table);
        }
        v
    }
}

fn apply_1d(rgb: &[f32; 3], size: usize, table: &[[f32; 3]]) -> [f32; 3] {
    let s = (size - 1) as f32;
    let mut out = [0.0; 3];
    for i in 0..3 {
        let x = rgb[i].clamp(0.0, 1.0) * s;
        let i0 = x.floor() as usize;
        let i1 = (i0 + 1).min(size - 1);
        let t = x - i0 as f32;
        let c0 = table[i0][i];
        let c1 = table[i1][i];
        out[i] = c0 + (c1 - c0) * t;
    }
    out
}

fn apply_3d(rgb: &[f32; 3], size: usize, table: &[[f32; 3]]) -> [f32; 3] {
    let n = size as i32;
    let s = (n - 1) as f32;
    let rx = (rgb[0].clamp(0.0, 1.0) * s).min(s);
    let gy = (rgb[1].clamp(0.0, 1.0) * s).min(s);
    let bz = (rgb[2].clamp(0.0, 1.0) * s).min(s);
    let x0 = rx.floor() as i32;
    let y0 = gy.floor() as i32;
    let z0 = bz.floor() as i32;
    let x1 = (x0 + 1).min(n - 1);
    let y1 = (y0 + 1).min(n - 1);
    let z1 = (z0 + 1).min(n - 1);
    let tx = rx - x0 as f32;
    let ty = gy - y0 as f32;
    let tz = bz - z0 as f32;

    let idx = |x: i32, y: i32, z: i32| -> usize {
        (z as usize * size * size) + (y as usize * size) + x as usize
    };

    let c000 = table[idx(x0, y0, z0)];
    let c100 = table[idx(x1, y0, z0)];
    let c010 = table[idx(x0, y1, z0)];
    let c110 = table[idx(x1, y1, z0)];
    let c001 = table[idx(x0, y0, z1)];
    let c101 = table[idx(x1, y0, z1)];
    let c011 = table[idx(x0, y1, z1)];
    let c111 = table[idx(x1, y1, z1)];

    let lerp = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let c00 = lerp(c000, c100, tx);
    let c10 = lerp(c010, c110, tx);
    let c01 = lerp(c001, c101, tx);
    let c11 = lerp(c011, c111, tx);
    let c0 = lerp(c00, c10, ty);
    let c1 = lerp(c01, c11, ty);
    lerp(c0, c1, tz)
}

pub fn parse_cube(text: &str) -> Result<Lut> {
    enum Section {
        None,
        Lut1D,
        Lut3D,
    }
    let mut section = Section::None;
    let mut shaper_size = 0usize;
    let mut shaper_table: Vec<[f32; 3]> = Vec::new();
    let mut cube_size = 0usize;
    let mut cube_table: Vec<[f32; 3]> = Vec::new();

    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if let Some(rest) = l.strip_prefix("LUT_1D_SIZE") {
            shaper_size = rest.trim().parse()?;
            section = Section::Lut1D;
            continue;
        }
        if let Some(rest) = l.strip_prefix("LUT_3D_SIZE") {
            cube_size = rest.trim().parse()?;
            section = Section::Lut3D;
            continue;
        }
        if l.starts_with("TITLE")
            || l.starts_with("DOMAIN_1D")
            || l.starts_with("DOMAIN_2D")
            || l.starts_with("DOMAIN_MIN")
            || l.starts_with("DOMAIN_MAX")
        {
            continue;
        }
        let parts: Vec<_> = l.split_whitespace().collect();
        if parts.len() == 3 {
            let r: f32 = parts[0].parse()?;
            let g: f32 = parts[1].parse()?;
            let b: f32 = parts[2].parse()?;
            match section {
                Section::Lut1D => shaper_table.push([r, g, b]),
                Section::Lut3D => cube_table.push([r, g, b]),
                Section::None => {}
            }
        }
    }

    if shaper_size > 0 && shaper_table.len() != shaper_size {
        return Err(anyhow!(".cube: invalid 1D table length"));
    }
    if cube_size > 0 && cube_table.len() != cube_size * cube_size * cube_size {
        return Err(anyhow!(".cube: invalid 3D table length"));
    }

    Ok(Lut {
        shaper_size,
        shaper_table,
        cube_size,
        cube_table,
    })
}

// ---- Utilities ----
fn apply_tone_map(rgb: [f32; 3], kind: ToneMapKind) -> [f32; 3] {
    match kind {
        ToneMapKind::None => rgb,
        ToneMapKind::Aces => {
            fn tm(x: f32) -> f32 {
                let a = 2.51;
                let b = 0.03;
                let c = 2.43;
                let d = 0.59;
                let e = 0.14;
                ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
            }
            [tm(rgb[0]), tm(rgb[1]), tm(rgb[2])]
        }
        ToneMapKind::Filmic => {
            fn tm(x: f32) -> f32 {
                let a = 0.15;
                let b = 0.50;
                let c = 0.10;
                let d = 0.20;
                let e = 0.02;
                let f = 0.30;
                let w = 11.2;
                let num = x * (a * x + c * b) + d * e;
                let den = x * (a * x + b) + d * f;
                let val = num / den - e / f;
                let num_w = w * (a * w + c * b) + d * e;
                let den_w = w * (a * w + b) + d * f;
                let white = num_w / den_w - e / f;
                (val / white).clamp(0.0, 1.0)
            }
            [tm(rgb[0]), tm(rgb[1]), tm(rgb[2])]
        }
    }
}

pub fn apply_gamma(rgb: [f32; 3], gamma: f32) -> [f32; 3] {
    if gamma <= 0.0001 {
        return rgb;
    }
    [
        rgb[0].powf(1.0 / gamma),
        rgb[1].powf(1.0 / gamma),
        rgb[2].powf(1.0 / gamma),
    ]
}

pub fn srgb_encode(v: f32) -> u8 {
    let x = v.max(0.0);
    let srgb = if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

// ---- LUT Generation (1D, Linear<->sRGB) ----
#[derive(Debug, Clone, Copy)]
pub enum ColorSpace {
    Linear,
    Srgb,
}

fn srgb_oetf(linear: f32) -> f32 {
    // linear->srgb
    if linear <= 0.0031308 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}
fn srgb_eotf(srgb: f32) -> f32 {
    // srgb->linear
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

pub fn make_1d_lut(src: ColorSpace, dst: ColorSpace, size: usize) -> String {
    let mut out = String::new();
    out.push_str("TITLE \"exrtool 1D LUT\"\n");
    out.push_str(&format!("LUT_1D_SIZE {}\n", size));
    out.push_str("DOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n");
    for i in 0..size {
        let x = (i as f32) / ((size - 1).max(1) as f32);
        let f = |v: f32| -> f32 {
            match (src, dst) {
                (ColorSpace::Linear, ColorSpace::Srgb) => srgb_oetf(v),
                (ColorSpace::Srgb, ColorSpace::Linear) => srgb_eotf(v),
                _ => v,
            }
        };
        let y = f(x).clamp(0.0, 1.0);
        out.push_str(&format!("{:.10} {:.10} {:.10}\n", y, y, y));
    }
    out
}

// ---- Color Primaries and 3D LUT generation ----
#[derive(Debug, Clone, Copy)]
pub enum Primaries {
    SrgbD65,       // sRGB / Rec.709 (D65)
    Rec2020D65,    // BT.2020 (D65)
    ACEScgD60,     // AP1 (D60)
    ACES2065_1D60, // AP0 (D60)
}

#[derive(Debug, Clone, Copy)]
pub enum TransferFn {
    Linear,
    Srgb,
    Gamma24,
    Gamma22,
}

#[derive(Debug, Clone, Copy)]
pub enum ClipMode {
    /// Clamp values to [0,1]
    Clip,
    /// Leave values as-is without clamping
    NoClip,
}

fn tf_encode(v: f64, tf: TransferFn) -> f64 {
    match tf {
        TransferFn::Linear => v,
        TransferFn::Srgb => {
            if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        }
        TransferFn::Gamma24 => v.max(0.0).powf(1.0 / 2.4),
        TransferFn::Gamma22 => v.max(0.0).powf(1.0 / 2.2),
    }
}
fn tf_decode(v: f64, tf: TransferFn) -> f64 {
    match tf {
        TransferFn::Linear => v,
        TransferFn::Srgb => {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        TransferFn::Gamma24 => v.max(0.0).powf(2.4),
        TransferFn::Gamma22 => v.max(0.0).powf(2.2),
    }
}

/// Apply 1D tone curve conversion from `src` transfer to `dst` transfer.
pub fn apply_tone_curve(rgb: [f32; 3], src: TransferFn, dst: TransferFn) -> [f32; 3] {
    [
        tf_encode(tf_decode(rgb[0] as f64, src), dst) as f32,
        tf_encode(tf_decode(rgb[1] as f64, src), dst) as f32,
        tf_encode(tf_decode(rgb[2] as f64, src), dst) as f32,
    ]
}

#[derive(Debug, Clone, Copy)]
struct Chromaticities {
    rx: f64,
    ry: f64,
    gx: f64,
    gy: f64,
    bx: f64,
    by: f64,
    wx: f64,
    wy: f64,
}

fn xy_to_xyz(x: f64, y: f64) -> Vector3<f64> {
    let x_xyz = x / y;
    let y_xyz = 1.0;
    let z_xyz = (1.0 - x - y) / y;
    Vector3::new(x_xyz, y_xyz, z_xyz)
}

fn primaries_of(p: Primaries) -> Chromaticities {
    match p {
        Primaries::SrgbD65 => Chromaticities {
            rx: 0.640,
            ry: 0.330,
            gx: 0.300,
            gy: 0.600,
            bx: 0.150,
            by: 0.060,
            wx: 0.3127,
            wy: 0.3290,
        },
        Primaries::Rec2020D65 => Chromaticities {
            rx: 0.708,
            ry: 0.292,
            gx: 0.170,
            gy: 0.797,
            bx: 0.131,
            by: 0.046,
            wx: 0.3127,
            wy: 0.3290,
        },
        Primaries::ACEScgD60 => Chromaticities {
            rx: 0.713,
            ry: 0.293,
            gx: 0.165,
            gy: 0.830,
            bx: 0.128,
            by: 0.044,
            wx: 0.32168,
            wy: 0.33767,
        },
        Primaries::ACES2065_1D60 => Chromaticities {
            rx: 0.73470,
            ry: 0.26530,
            gx: 0.00000,
            gy: 1.00000,
            bx: 0.00010,
            by: -0.07700,
            wx: 0.32168,
            wy: 0.33767,
        },
    }
}

fn rgb_to_xyz_matrix(p: Primaries) -> Matrix3<f64> {
    let c = primaries_of(p);
    let xr = xy_to_xyz(c.rx, c.ry);
    let xg = xy_to_xyz(c.gx, c.gy);
    let xb = xy_to_xyz(c.bx, c.by);
    let w = xy_to_xyz(c.wx, c.wy);
    let m = Matrix3::from_columns(&[xr, xg, xb]);
    let s = m.try_inverse().unwrap() * w; // solve for scaling factors
    m * Matrix3::from_diagonal(&s)
}

fn bradford_adapt_matrix(src_wp: Vector3<f64>, dst_wp: Vector3<f64>) -> Matrix3<f64> {
    // Bradford matrices
    let m = Matrix3::new(
        0.8951, 0.2664, -0.1614, -0.7502, 1.7135, 0.0367, 0.0389, -0.0685, 1.0296,
    );
    let m_inv = Matrix3::new(
        0.9869929, -0.1470543, 0.1599627, 0.4323053, 0.5183603, 0.0492912, -0.0085287, 0.0400428,
        0.9684867,
    );
    let src_lms = m * src_wp;
    let dst_lms = m * dst_wp;
    let d = Matrix3::from_diagonal(&Vector3::new(
        dst_lms.x / src_lms.x,
        dst_lms.y / src_lms.y,
        dst_lms.z / src_lms.z,
    ));
    m_inv * d * m
}

fn xyz_white(p: Primaries) -> Vector3<f64> {
    let c = primaries_of(p);
    xy_to_xyz(c.wx, c.wy)
}

fn rgb_to_rgb_matrix(src: Primaries, dst: Primaries) -> Matrix3<f64> {
    let m_src = rgb_to_xyz_matrix(src);
    let m_dst = rgb_to_xyz_matrix(dst);
    let a = if primaries_of(src).wx == primaries_of(dst).wx
        && primaries_of(src).wy == primaries_of(dst).wy
    {
        Matrix3::identity()
    } else {
        bradford_adapt_matrix(xyz_white(src), xyz_white(dst))
    };
    m_dst.try_inverse().unwrap() * a * m_src
}

pub fn make_3d_lut_cube(
    src_prim: Primaries,
    src_tf: TransferFn,
    dst_prim: Primaries,
    dst_tf: TransferFn,
    size: usize,
    shaper_size: usize,
) -> String {
    make_3d_lut_cube_progress(
        src_prim,
        src_tf,
        dst_prim,
        dst_tf,
        size,
        shaper_size,
        |_| true,
    )
    .expect("make_3d_lut_cube_progress should not fail")
}

pub fn make_3d_lut_cube_progress<F>(
    src_prim: Primaries,
    src_tf: TransferFn,
    dst_prim: Primaries,
    dst_tf: TransferFn,
    size: usize,
    shaper_size: usize,
    progress: F,
) -> Result<String, String>
where
    F: Fn(f64) -> bool + Sync,
{
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let m = rgb_to_rgb_matrix(src_prim, dst_prim);
    let mut out = String::new();
    out.push_str("TITLE \"exrtool 3D LUT\"\n");
    if shaper_size > 0 {
        out.push_str(&format!("LUT_1D_SIZE {}\n", shaper_size));
        out.push_str("DOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n");
        for i in 0..shaper_size {
            let x = i as f32 / ((shaper_size - 1).max(1) as f32);
            let y = apply_tone_curve([x, x, x], src_tf, TransferFn::Linear)[0] as f64;
            out.push_str(&format!("{:.10} {:.10} {:.10}\n", y, y, y));
        }
    }
    out.push_str(&format!("LUT_3D_SIZE {}\n", size));
    out.push_str("DOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n");
    let denom = (size - 1).max(1) as f64;
    let total = size * size * size;
    let counter = AtomicUsize::new(0);
    let cancelled = AtomicBool::new(false);
    let progress = &progress;
    let lines = (0..total)
        .into_par_iter()
        .try_fold(Vec::new, |mut chunk, i| {
            if cancelled.load(Ordering::Relaxed) {
                return Err(());
            }
            let r = i % size;
            let g = (i / size) % size;
            let b = i / (size * size);
            let rf = r as f64 / denom;
            let gf = g as f64 / denom;
            let bf = b as f64 / denom;
            let rs = tf_decode(rf, src_tf);
            let gs = tf_decode(gf, src_tf);
            let bs = tf_decode(bf, src_tf);
            let v = Vector3::new(rs, gs, bs);
            let v_lin_dst = m * v;
            let rd = tf_encode(v_lin_dst.x, dst_tf).clamp(0.0, 1.0);
            let gd = tf_encode(v_lin_dst.y, dst_tf).clamp(0.0, 1.0);
            let bd = tf_encode(v_lin_dst.z, dst_tf).clamp(0.0, 1.0);
            chunk.push(format!("{:.10} {:.10} {:.10}\n", rd, gd, bd));
            let c = counter.fetch_add(1, Ordering::Relaxed) + 1;
            if c % 1000 == 0 || c == total {
                let pct = c as f64 / total as f64 * 100.0;
                if !progress(pct) {
                    cancelled.store(true, Ordering::Relaxed);
                    return Err(());
                }
            }
            Ok(chunk)
        })
        .try_reduce(Vec::new, |mut a, mut b| {
            a.append(&mut b);
            Ok(a)
        })
        .map_err(|_| "cancelled".to_string())?;
    for line in lines {
        out.push_str(&line);
    }
    Ok(out)
}

// ---- Rule Application ----
#[derive(Debug, Deserialize)]
pub struct ApplyRule {
    pub input: PathBuf,
    #[serde(default)]
    pub output: Option<PathBuf>,
    #[serde(default)]
    pub max_size: Option<u32>,
    #[serde(default)]
    pub exposure: Option<f32>,
    #[serde(default)]
    pub gamma: Option<f32>,
    #[serde(default)]
    pub lut: Option<PathBuf>,
}

pub fn apply_rules_file(path: &Path, dry_run: bool, backup: bool) -> Result<()> {
    let text = fs::read_to_string(path)?;
    let rules: Vec<ApplyRule> = serde_yaml::from_str(&text)?;
    for r in rules {
        let input = r.input;
        let out = r
            .output
            .clone()
            .unwrap_or_else(|| input.with_extension("png"));
        if dry_run {
            println!("process: {} -> {}", input.display(), out.display());
            continue;
        }
        let img = load_exr_basic(&input)?;
        let lut_obj = if let Some(ref p) = r.lut {
            let txt = fs::read_to_string(p)?;
            Some(parse_cube(&txt)?)
        } else {
            None
        };
        let preview = generate_preview(
            &img,
            r.max_size.unwrap_or(2048),
            r.exposure.unwrap_or(0.0),
            r.gamma.unwrap_or(2.2),
            lut_obj.as_ref(),
            PreviewQuality::High,
        );
        if backup && out.exists() {
            let bak = out.with_extension("bak");
            fs::copy(&out, &bak)?;
        }
        export_png(&out, &preview)?;
        println!("saved: {} -> {}", input.display(), out.display());
    }
    Ok(())
}
