use std::{env, fs, io::Write, path::PathBuf};

fn ensure_icon() {
  let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
  let icons_dir = manifest_dir.join("icons");
  let icon_path = icons_dir.join("icon.ico");

  let _ = fs::create_dir_all(&icons_dir);

  // Build minimal ICO with BMP (1x1, BGRA32, fully transparent)
  // ICONDIR (6) + ICONDIRENTRY (16) + BITMAPINFOHEADER (40) + XOR(4) + AND(4)
  let bytes_in_res: u32 = 40 + 4 + 4;
  let image_offset: u32 = 6 + 16;

  let mut ico: Vec<u8> = Vec::with_capacity((image_offset + bytes_in_res) as usize);
  // ICONDIR
  ico.extend_from_slice(&[0x00, 0x00]); // reserved
  ico.extend_from_slice(&[0x01, 0x00]); // type = icon
  ico.extend_from_slice(&[0x01, 0x00]); // count = 1
  // ICONDIRENTRY
  ico.push(0x01); // width = 1
  ico.push(0x01); // height = 1
  ico.push(0x00); // color count = 0
  ico.push(0x00); // reserved
  ico.extend_from_slice(&1u16.to_le_bytes());    // planes = 1
  ico.extend_from_slice(&32u16.to_le_bytes());   // bit count = 32
  ico.extend_from_slice(&bytes_in_res.to_le_bytes()); // bytes in resource
  ico.extend_from_slice(&image_offset.to_le_bytes()); // offset

  // BITMAPINFOHEADER (40 bytes)
  let mut bih: Vec<u8> = Vec::with_capacity(40);
  bih.extend_from_slice(&40u32.to_le_bytes()); // biSize
  bih.extend_from_slice(&1i32.to_le_bytes());  // biWidth = 1
  bih.extend_from_slice(&(2i32).to_le_bytes()); // biHeight = 2 (XOR+AND)
  bih.extend_from_slice(&1u16.to_le_bytes());  // biPlanes = 1
  bih.extend_from_slice(&32u16.to_le_bytes()); // biBitCount = 32
  bih.extend_from_slice(&0u32.to_le_bytes());  // biCompression = BI_RGB
  bih.extend_from_slice(&4u32.to_le_bytes());  // biSizeImage = 4 bytes (XOR)
  bih.extend_from_slice(&0u32.to_le_bytes());  // biXPelsPerMeter
  bih.extend_from_slice(&0u32.to_le_bytes());  // biYPelsPerMeter
  bih.extend_from_slice(&0u32.to_le_bytes());  // biClrUsed
  bih.extend_from_slice(&0u32.to_le_bytes());  // biClrImportant

  // XOR bitmap (BGRA), 1x1 pixel, fully transparent
  let xor = [0u8, 0u8, 0u8, 0u8];
  // AND mask (1bpp, padded to 32 bits per row) -> 4 bytes zero
  let and_mask = [0u8, 0u8, 0u8, 0u8];

  ico.extend_from_slice(&bih);
  ico.extend_from_slice(&xor);
  ico.extend_from_slice(&and_mask);

  let mut f = fs::File::create(&icon_path).expect("create icon.ico");
  f.write_all(&ico).expect("write icon.ico");
}

fn main() {
  ensure_icon();
  tauri_build::build()
}
