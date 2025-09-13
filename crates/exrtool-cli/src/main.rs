use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use exrtool_core::{export_png, generate_preview, load_exr_basic, parse_cube, make_1d_lut, ColorSpace};
use std::path::PathBuf;
use std::fs;

#[derive(Parser)]
#[command(name = "exrtool")] 
#[command(about = "EXR プレビューとピクセル検査のCLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// プレビューPNGを書き出し
    Preview {
        /// 入力EXR
        input: PathBuf,
        /// 出力PNGパス
        #[arg(short, long)]
        out: PathBuf,
        /// 収まる最大サイズ
        #[arg(long, default_value_t = 2048)]
        max_size: u32,
        /// 露出（stop単位）
        #[arg(long, default_value_t = 0.0)]
        exposure: f32,
        /// ガンマ（0で無効）
        #[arg(long, default_value_t = 2.2)]
        gamma: f32,
        /// .cubeファイル（任意）
        #[arg(long)]
        lut: Option<PathBuf>,
    },

    /// 指定座標のリニア値を表示
    Probe {
        /// 入力EXR
        input: PathBuf,
        /// x座標
        #[arg(long)]
        x: usize,
        /// y座標
        #[arg(long)]
        y: usize,
    },

    /// 1D LUT(.cube) を生成（トーンカーブ変換）
    MakeLut1D {
        /// 変換元: linear | srgb
        #[arg(long)]
        src: String,
        /// 変換先: linear | srgb
        #[arg(long)]
        dst: String,
        /// テーブルサイズ（例: 1024）
        #[arg(long, default_value_t = 1024)]
        size: usize,
        /// 出力パス（.cube）
        #[arg(short, long)]
        out: PathBuf,
    },

    /// 3D LUT(.cube) を生成（色域+トーン変換）
    MakeLut3D {
        /// 変換元primaries: srgb | rec2020 | acescg | aces2065
        #[arg(long)]
        src_space: String,
        /// 変換元トーン: linear | srgb | g24 | g22
        #[arg(long, default_value = "linear")]
        src_tf: String,
        /// 変換先primaries: srgb | rec2020 | acescg | aces2065
        #[arg(long)]
        dst_space: String,
        /// 変換先トーン: linear | srgb | g24 | g22
        #[arg(long, default_value = "srgb")]
        dst_tf: String,
        /// テーブルサイズ（既定: 33）
        #[arg(long, default_value_t = 33)]
        size: usize,
        /// クリップモード: clip | noclip
        #[arg(long, default_value = "clip")]
        clip_mode: String,
        /// 出力パス（.cube）
        #[arg(short, long)]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Preview { input, out, max_size, exposure, gamma, lut } => {
            let img = load_exr_basic(&input)?;
            let lut_obj = if let Some(p) = lut { 
                let txt = fs::read_to_string(p)?; 
                Some(parse_cube(&txt)?)
            } else { None };
            let preview = generate_preview(&img, max_size, exposure, gamma, lut_obj.as_ref());
            export_png(&out, &preview)?;
            println!("w={} h={} => {}", preview.width, preview.height, out.display());
        }
        Commands::Probe { input, x, y } => {
            let img = load_exr_basic(&input)?;
            let px = img.get_linear(x, y).with_context(|| format!("座標が範囲外: {},{}", x, y))?;
            println!("linear RGBA: {:.7} {:.7} {:.7} {:.7}", px.r, px.g, px.b, px.a);
        }
        Commands::MakeLut1D { src, dst, size, out } => {
            let parse_cs = |s:&str| -> Result<ColorSpace> {
                match s.to_ascii_lowercase().as_str() {
                    "linear" => Ok(ColorSpace::Linear),
                    "srgb" => Ok(ColorSpace::Srgb),
                    _ => Err(anyhow::anyhow!("unknown colorspace: {}", s))
                }
            };
            let cs_src = parse_cs(&src)?;
            let cs_dst = parse_cs(&dst)?;
            let text = make_1d_lut(cs_src, cs_dst, size);
            fs::write(&out, text)?;
            println!("LUT saved: {} ({} -> {}, size={})", out.display(), src, dst, size);
        }
        Commands::MakeLut3D { src_space, src_tf, dst_space, dst_tf, size, clip_mode, out } => {
            use exrtool_core::{Primaries, TransferFn, ClipMode, make_3d_lut_cube};
            let parse_space = |s:&str| -> Result<Primaries> { match s.to_ascii_lowercase().as_str() {
                "srgb"|"rec709" => Ok(Primaries::SrgbD65),
                "rec2020"|"bt2020" => Ok(Primaries::Rec2020D65),
                "acescg"|"ap1" => Ok(Primaries::ACEScgD60),
                "aces2065"|"ap0"|"aces" => Ok(Primaries::ACES2065_1D60),
                _ => Err(anyhow::anyhow!("unknown space: {}", s)) } };
            let parse_tf = |s:&str| -> Result<TransferFn> { match s.to_ascii_lowercase().as_str() {
                "linear" => Ok(TransferFn::Linear),
                "srgb" => Ok(TransferFn::Srgb),
                "g24"|"gamma2.4" => Ok(TransferFn::Gamma24),
                "g22"|"gamma2.2" => Ok(TransferFn::Gamma22),
                _ => Err(anyhow::anyhow!("unknown transfer: {}", s)) } };
            let parse_clip = |s:&str| -> Result<ClipMode> { match s.to_ascii_lowercase().as_str() {
                "clip" => Ok(ClipMode::Clip),
                "noclip"|"none" => Ok(ClipMode::NoClip),
                _ => Err(anyhow::anyhow!("unknown clip mode: {}", s)) } };
            let sp = parse_space(&src_space)?; let dt = parse_space(&dst_space)?;
            let st = parse_tf(&src_tf)?; let tt = parse_tf(&dst_tf)?;
            let cm = parse_clip(&clip_mode)?;
            let text = make_3d_lut_cube(sp, st, dt, tt, size, cm);
            fs::write(&out, text)?;
            println!(
                "3D LUT saved: {} ({} {} -> {} {}, size={}, clip={})",
                out.display(), src_space, src_tf, dst_space, dst_tf, size, clip_mode
            );
        }
    }
    Ok(())
}
