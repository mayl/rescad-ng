//! Embeds the icon and version information into Windows executables.

use std::path::Path;
use std::{env, fs};

#[path = "src/icon.rs"]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out = env::var("OUT_DIR").unwrap();
    let ico = Path::new(&out).join("rescad-ng.ico");
    fs::write(&ico, ico_file(&[16, 24, 32, 48, 64, 128, 256])).unwrap();

    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts: Vec<u16> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    parts.resize(4, 0);
    let v = format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3]);
    let rc = format!(
        r#"1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {v}
PRODUCTVERSION {v}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "ResCAD NG resistor calculator"
      VALUE "FileVersion", "{version}"
      VALUE "ProductName", "ResCAD NG"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "rescad-ng.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        ico = ico.display().to_string().replace('\\', "/"),
    );
    let rc_path = Path::new(&out).join("rescad-ng.rc");
    fs::write(&rc_path, rc).unwrap();
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}

/// Build a multi-resolution .ico using 32-bit BMP entries.
fn ico_file(sizes: &[usize]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| bmp_entry(s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (&s, img) in sizes.iter().zip(&images) {
        let dim = if s >= 256 { 0 } else { s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += img.len();
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

fn bmp_entry(size: usize) -> Vec<u8> {
    let rgba = icon::render(size);
    let mut out = Vec::new();
    // BITMAPINFOHEADER; the height counts both the colour and the AND mask.
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0u8; 24]);
    // Pixels, bottom-up BGRA.
    for y in (0..size).rev() {
        for x in 0..size {
            let i = (y * size + x) * 4;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    // AND mask: all zero (alpha channel is used), rows padded to 32 bits.
    let row = size.div_ceil(32) * 4;
    out.extend(std::iter::repeat_n(0u8, row * size));
    out
}
