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

    /// 1D LUT(.cube) を生成（Linear<->sRGB）
    MakeLut {
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
    }
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
        Commands::MakeLut { src, dst, size, out } => {
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
    }
    Ok(())
}
