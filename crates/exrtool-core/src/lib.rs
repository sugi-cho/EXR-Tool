use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

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

#[derive(Debug)]
pub struct LoadedExr {
    pub width: usize,
    pub height: usize,
    // interleaved RGBA (linear, f32; a=1.0 if absent)
    pub rgba_f32: Vec<f32>,
}

impl LoadedExr {
    pub fn get_linear(&self, x: usize, y: usize) -> Option<LinearPixel> {
        if x >= self.width || y >= self.height { return None; }
        let idx = (y * self.width + x) * 4;
        Some(LinearPixel{
            r: self.rgba_f32[idx + 0],
            g: self.rgba_f32[idx + 1],
            b: self.rgba_f32[idx + 2],
            a: self.rgba_f32[idx + 3],
        })
    }
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
    Ok(LoadedExr { width: w as usize, height: h as usize, rgba_f32: data })
}

// ---- Preview Generation ----
pub fn generate_preview(
    img: &LoadedExr,
    max_size: u32,
    exposure: f32,
    gamma: f32,
    lut: Option<&Lut>,
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

    for oy in 0..out_h {
        for ox in 0..out_w {
            let src_x = ((ox as f32) / scale).floor().clamp(0.0, (w - 1) as f32) as usize;
            let src_y = ((oy as f32) / scale).floor().clamp(0.0, (h - 1) as f32) as usize;
            let si = (src_y * img.width + src_x) * 4;
            let mut r = img.rgba_f32[si + 0];
            let mut g = img.rgba_f32[si + 1];
            let mut b = img.rgba_f32[si + 2];
            let a = img.rgba_f32[si + 3];

            // exposure in stops (2^exposure)
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
    }

    PreviewImage { width: out_w, height: out_h, rgba8 }
}

pub fn export_png(path: &Path, preview: &PreviewImage) -> Result<()> {
    let img = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8.clone())
        .ok_or_else(|| anyhow!("failed to create image buffer"))?;
    image::DynamicImage::ImageRgba8(img).save(path)?;
    Ok(())
}

// ---- LUT (.cube minimal) ----
#[derive(Debug, Clone)]
pub enum LutKind { Lut1D, Lut3D }

#[derive(Debug, Clone)]
pub struct Lut {
    kind: LutKind,
    size: usize,
    table: Vec<[f32;3]>, // 1D: size entries, 3D: size^3 entries (r-major)
}

impl Lut {
    pub fn apply(&self, rgb: [f32;3]) -> [f32;3] {
        match self.kind {
            LutKind::Lut1D => self.apply_1d(rgb),
            LutKind::Lut3D => self.apply_3d(rgb),
        }
    }

    fn apply_1d(&self, rgb: [f32;3]) -> [f32;3] {
        let s = (self.size - 1) as f32;
        let mut out = [0.0;3];
        for i in 0..3 {
            let x = rgb[i].clamp(0.0, 1.0) * s;
            let i0 = x.floor() as usize;
            let i1 = (i0 + 1).min(self.size - 1);
            let t = x - i0 as f32;
            let c0 = self.table[i0][i];
            let c1 = self.table[i1][i];
            out[i] = c0 + (c1 - c0) * t;
        }
        out
    }

    fn apply_3d(&self, rgb: [f32;3]) -> [f32;3] {
        let n = self.size as i32;
        let s = (n - 1) as f32;
        let rx = (rgb[0].clamp(0.0, 1.0) * s).min(s);
        let gy = (rgb[1].clamp(0.0, 1.0) * s).min(s);
        let bz = (rgb[2].clamp(0.0, 1.0) * s).min(s);
        let x0 = rx.floor() as i32; let y0 = gy.floor() as i32; let z0 = bz.floor() as i32;
        let x1 = (x0 + 1).min(n-1); let y1 = (y0 + 1).min(n-1); let z1 = (z0 + 1).min(n-1);
        let tx = rx - x0 as f32; let ty = gy - y0 as f32; let tz = bz - z0 as f32;

        let idx = |x:i32,y:i32,z:i32| -> usize {
            // r-major: r changes fastest: idx = z*n*n + y*n + x
            (z as usize * self.size * self.size) + (y as usize * self.size) + x as usize
        };

        let c000 = self.table[idx(x0,y0,z0)];
        let c100 = self.table[idx(x1,y0,z0)];
        let c010 = self.table[idx(x0,y1,z0)];
        let c110 = self.table[idx(x1,y1,z0)];
        let c001 = self.table[idx(x0,y0,z1)];
        let c101 = self.table[idx(x1,y0,z1)];
        let c011 = self.table[idx(x0,y1,z1)];
        let c111 = self.table[idx(x1,y1,z1)];

        let lerp = |a:[f32;3],b:[f32;3],t:f32| [
            a[0]+(b[0]-a[0])*t,
            a[1]+(b[1]-a[1])*t,
            a[2]+(b[2]-a[2])*t
        ];
        let c00 = lerp(c000,c100,tx); let c10 = lerp(c010,c110,tx);
        let c01 = lerp(c001,c101,tx); let c11 = lerp(c011,c111,tx);
        let c0 = lerp(c00,c10,ty); let c1 = lerp(c01,c11,ty);
        lerp(c0,c1,tz)
    }
}

pub fn parse_cube(text: &str) -> Result<Lut> {
    let mut size: Option<usize> = None;
    let mut table: Vec<[f32;3]> = Vec::new();
    let mut kind = LutKind::Lut1D;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') { continue; }
        if let Some(rest) = l.strip_prefix("LUT_1D_SIZE") { 
            size = Some(rest.trim().parse()?);
            kind = LutKind::Lut1D;
            continue;
        }
        if let Some(rest) = l.strip_prefix("LUT_3D_SIZE") {
            size = Some(rest.trim().parse()?);
            kind = LutKind::Lut3D;
            continue;
        }
        if l.starts_with("TITLE") || l.starts_with("DOMAIN_1D") || l.starts_with("DOMAIN_2D") || l.starts_with("DOMAIN_MIN") || l.starts_with("DOMAIN_MAX") {
            continue;
        }
        let parts: Vec<_> = l.split_whitespace().collect();
        if parts.len() == 3 {
            let r: f32 = parts[0].parse()?;
            let g: f32 = parts[1].parse()?;
            let b: f32 = parts[2].parse()?;
            table.push([r,g,b]);
        }
    }
    let size = size.ok_or_else(|| anyhow!(".cube: missing LUT size"))?;
    if matches!(kind, LutKind::Lut3D) {
        if table.len() != size*size*size { return Err(anyhow!(".cube: invalid 3D table length")); }
    } else {
        if table.len() != size { return Err(anyhow!(".cube: invalid 1D table length")); }
    }
    Ok(Lut{ kind, size, table })
}

// ---- Utilities ----
pub fn apply_gamma(rgb: [f32;3], gamma: f32) -> [f32;3] {
    if gamma <= 0.0001 { return rgb; }
    [rgb[0].powf(1.0/gamma), rgb[1].powf(1.0/gamma), rgb[2].powf(1.0/gamma)]
}

pub fn srgb_encode(v: f32) -> u8 {
    let x = v.max(0.0);
    let srgb = if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0/2.4) - 0.055 };
    (srgb.clamp(0.0,1.0) * 255.0 + 0.5).floor() as u8
}
