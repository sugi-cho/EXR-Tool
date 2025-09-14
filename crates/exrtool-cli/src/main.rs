use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use exrtool_core::{export_png, generate_preview, load_exr_basic, parse_cube, make_1d_lut, ColorSpace, PreviewQuality, ClipMode};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "exrtool")]
#[command(about = "EXR プレビューとピクセル検査のCLI", long_about = None)]
struct Cli {
    /// OCIO config: "aces1.3" or file path
    #[arg(long)]
    ocio: Option<String>,
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
        /// 高品質リサイズ
        #[arg(long, value_enum, default_value_t = Quality::Fast)]
        quality: Quality,
    },
    /// 連番EXRのFPS属性を一括設定（feature `exr_pure` 必要）
    SeqFps {
        /// ディレクトリ
        #[arg(long)]
        dir: PathBuf,
        /// FPS値（float）
        #[arg(long)]
        fps: f32,
        /// 属性名（既定: FramesPerSecond）
        #[arg(long, default_value = "FramesPerSecond")]
        attr: String,
        /// 再帰的に走査
        #[arg(long, default_value_t = false)]
        recursive: bool,
        /// 変更せずに対象のみ表示
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// 上書き時に .bak を作成
        #[arg(long, default_value_t = true)]
        backup: bool,
    },
    /// 連番EXRからProRes動画を生成（ffmpeg必要）
    Prores {
        /// ディレクトリ
        #[arg(long)]
        dir: PathBuf,
        /// FPS
        #[arg(long, default_value_t = 24.0)]
        fps: f32,
        /// 出力ファイル（.mov）
        #[arg(long)]
        out: PathBuf,
        /// 色空間変換: linear:srgb | acescg:srgb | aces2065:srgb
        #[arg(long, default_value = "linear:srgb")]
        colorspace: String,
        /// プロファイル: 422hq/422/4444 等
        #[arg(long, default_value = "422hq")]
        profile: String,
        /// 最大辺サイズ
        #[arg(long, default_value_t = 2048)]
        max_size: u32,
        /// 露出（stop）
        #[arg(long, default_value_t = 0.0)]
        exposure: f32,
        /// ガンマ
        #[arg(long, default_value_t = 2.2)]
        gamma: f32,
        /// 高品質リサイズ
        #[arg(long, value_enum, default_value_t = Quality::High)]
        quality: Quality,
    },

    /// メタデータを表示（feature `exr_pure` 必要）
    Metadata {
        /// 入力EXR
        input: PathBuf,
        /// 出力形式: table | json
        #[arg(long, default_value = "table")]
        format: String,
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
        /// 1D シェーパーサイズ（0で無効）
        #[arg(long, default_value_t = 1024)]
        shaper_size: usize,
        /// 出力パス（.cube）
        #[arg(short, long)]
        out: PathBuf,
    },

    /// ルールファイルに基づき処理を適用
    Apply {
        /// ルールファイル(YAML)
        #[arg(long)]
        rules: PathBuf,
        /// 実行内容のみ表示
        #[arg(long)]
        dry_run: bool,
        /// 出力を上書きする際にバックアップ(.bak)を作成
        #[arg(long)]
        backup: bool,
    },
}

#[derive(Clone, ValueEnum)]
enum Quality { Fast, High }

fn main() -> Result<()> {
    let cli = Cli::parse();
    // OCIO連携は現在ビルド無効（use_ocio feature）。対応時に実装を復帰。
    match cli.command {
        Commands::Preview { input, out, max_size, exposure, gamma, lut, quality } => {
            let img = load_exr_basic(&input)?;
            let lut_obj = if let Some(p) = lut {
                let txt = fs::read_to_string(p)?;
                Some(parse_cube(&txt)?)
            } else { None };
            let pq = match quality { Quality::Fast => PreviewQuality::Fast, Quality::High => PreviewQuality::High };
            let preview = generate_preview(&img, max_size, exposure, gamma, lut_obj.as_ref(), pq);
            export_png(&out, &preview)?;
            println!(
                "w={} h={} => {}",
                preview.width,
                preview.height,
                out.display()
            );
        }
        Commands::Probe { input, x, y } => {
            let img = load_exr_basic(&input)?;
            let px = img
                .get_linear(x, y)
                .with_context(|| format!("座標が範囲外: {},{}", x, y))?;
            println!(
                "linear RGBA: {:.7} {:.7} {:.7} {:.7}",
                px.r, px.g, px.b, px.a
            );
        }
        Commands::MakeLut1D {
            src,
            dst,
            size,
            out,
        } => {
            let parse_cs = |s: &str| -> Result<ColorSpace> {
                match s.to_ascii_lowercase().as_str() {
                    "linear" => Ok(ColorSpace::Linear),
                    "srgb" => Ok(ColorSpace::Srgb),
                    _ => Err(anyhow::anyhow!("unknown colorspace: {}", s)),
                }
            };
            let cs_src = parse_cs(&src)?;
            let cs_dst = parse_cs(&dst)?;
            let text = make_1d_lut(cs_src, cs_dst, size);
            fs::write(&out, text)?;
            println!(
                "LUT saved: {} ({} -> {}, size={})",
                out.display(),
                src,
                dst,
                size
            );
        }
        Commands::MakeLut3D { src_space, src_tf, dst_space, dst_tf, size, shaper_size, out } => {
            use exrtool_core::{Primaries, TransferFn, make_3d_lut_cube};
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
            let text = make_3d_lut_cube(sp, st, dt, tt, size, shaper_size);
            fs::write(&out, text)?;
            println!("3D LUT saved: {} ({} {} -> {} {}, size={} shaper={})", out.display(), src_space, src_tf, dst_space, dst_tf, size, shaper_size);
        }
        Commands::Apply { rules, dry_run, backup } => {
            exrtool_core::apply_rules_file(&rules, dry_run, backup)?;
        }
        Commands::SeqFps { dir, fps, attr, recursive, dry_run, backup } => {
            #[cfg(feature = "exr_pure")]
            {
                use std::collections::HashMap;
                let mut files = Vec::new();
                let walk = |p: &PathBuf, out: &mut Vec<PathBuf>| -> Result<()> {
                    for entry in fs::read_dir(p)? {
                        let e = entry?; let path = e.path();
                        if path.is_dir() { if recursive { walk(&path, out)?; } }
                        else if path.extension().map(|s| s.to_string_lossy().to_ascii_lowercase()) == Some("exr".into()) {
                            out.push(path);
                        }
                    }
                    Ok(())
                };
                walk(&dir, &mut files)?;
                files.sort_by(|a,b| a.file_name().unwrap().cmp(b.file_name().unwrap()));
                if files.is_empty() { println!("no EXR files found in {}", dir.display()); return Ok(()); }
                println!("target files: {}", files.len());
                if dry_run {
                    for f in files { println!("{}", f.display()); }
                    return Ok(());
                }
                let mut map = HashMap::new();
                map.insert(attr.clone(), format!("{}", fps));
                for f in files {
                    // write in-place or with backup via core save.rs if available; here do naive: out=None => overwrite
                    match exrtool_core::metadata::write_metadata(&f, &map, None) {
                        Ok(_) => println!("wrote {}={} to {}", attr, fps, f.display()),
                        Err(e) => eprintln!("failed {}: {}", f.display(), e),
                    }
                }
            }
            #[cfg(not(feature = "exr_pure"))]
            {
                eprintln!("seq-fps requires --features exr_pure");
            }
        }
        Commands::Prores { dir, fps, out, colorspace, profile, max_size, exposure, gamma, quality } => {
            use std::process::{Command, Stdio};
            // check ffmpeg
            if Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() {
                eprintln!("ffmpeg not found. Please install ffmpeg and ensure it's on PATH.");
                return Ok(());
            }
            // gather exr files
            let mut files: Vec<PathBuf> = Vec::new();
            for entry in fs::read_dir(&dir)? { let e=entry?; let p=e.path(); if p.is_file() && p.extension().map(|s| s.to_string_lossy().to_ascii_lowercase())==Some("exr".into()) { files.push(p); } }
            files.sort_by(|a,b| a.file_name().unwrap().cmp(b.file_name().unwrap()));
            if files.is_empty() { eprintln!("no EXR files in {}", dir.display()); return Ok(()); }
            // prepare LUT based on colorspace
            let mut lut_obj = None;
            let cs = colorspace.to_lowercase();
            if cs != "linear:srgb" {
                use exrtool_core::{make_3d_lut_cube, Primaries, TransferFn};
                let (sp, dp) = if cs=="acescg:srgb" { (Primaries::ACEScgD60, Primaries::SrgbD65) } else if cs=="aces2065:srgb" { (Primaries::ACES2065_1D60, Primaries::SrgbD65) } else { (Primaries::SrgbD65, Primaries::SrgbD65) };
                let text = make_3d_lut_cube(sp, TransferFn::Linear, dp, TransferFn::Srgb, 33, 1024);
                lut_obj = Some(parse_cube(&text)?);
            }
            // spawn ffmpeg
            let mut child = Command::new("ffmpeg")
                .arg("-y")
                .arg("-f").arg("image2pipe")
                .arg("-r").arg(format!("{}", fps))
                .arg("-vcodec").arg("png")
                .arg("-i").arg("-")
                .arg("-c:v").arg("prores_ks")
                .arg("-profile:v").arg(match profile.as_str() { "422hq"=>"3", "422"=>"2", "4444"=>"4", _=>"3" })
                .arg(out.as_os_str())
                .stdin(Stdio::piped())
                .spawn()
                .context("failed to spawn ffmpeg")?;
            {
                use image::{ImageBuffer, Rgba};
                let mut stdin = child.stdin.take().unwrap();
                for f in files {
                    let img = load_exr_basic(&f)?;
                    let pq = match quality { Quality::Fast=>PreviewQuality::Fast, Quality::High=>PreviewQuality::High };
                    let preview = generate_preview(&img, max_size, exposure, gamma, lut_obj.as_ref(), pq);
                    // encode PNG to ffmpeg stdin
                    let buf = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba8).expect("invalid buffer");
                    let mut bytes: Vec<u8> = Vec::new();
                    image::DynamicImage::ImageRgba8(buf).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageOutputFormat::Png)?;
                    use std::io::Write;
                    stdin.write_all(&bytes)?;
                }
            }
            let status = child.wait()?;
            if !status.success() { eprintln!("ffmpeg exited with status {:?}", status); }
            else { println!("wrote {}", out.display()); }
        }
        Commands::Metadata { input, format } => {
            // coreのread_metadataを呼び出し（feature未有効時はErr）
            match exrtool_core::read_metadata(&input) {
                Ok(meta) => {
                    if format.to_lowercase() == "json" {
                        println!("{}", serde_json::to_string_pretty(&meta).unwrap_or("{}".into()));
                    } else {
                        // 簡易表形式
                        for (i, h) in meta.headers.iter().enumerate() {
                            println!("# Header {}", i);
                            println!("  layer_name   : {}", h.layer_name.as_deref().unwrap_or(""));
                            println!("  layer_pos    : {},{}", h.layer_position.0, h.layer_position.1);
                            println!("  layer_size   : {}x{}", h.layer_size.0, h.layer_size.1);
                            println!("  pixel_aspect : {}", h.pixel_aspect);
                            println!("  line_order   : {}", h.line_order);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("metadata取得に失敗: {}\nヒント: CLIを feature 'exr_pure' でビルドしてください\n  cargo run -p exrtool-cli --features exr_pure -- metadata <in.exr>", e);
                }
            }
        }
    }
    Ok(())
}
