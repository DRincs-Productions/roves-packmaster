//! Derives a Windows `.ico` and a macOS `.icns` from the single PNG icon a game developer
//! picks in "Release info" (see `bundle.rs`'s own `apply_icon`) — consolidating what used to
//! be two separate pickers (a PNG for the runtime window icon, a pre-made `.ico` for the
//! packaged `.exe`'s own icon resource) into one source image applied everywhere it's
//! possible to apply it.
//!
//! Both container formats accept plain PNG-encoded frames directly, at their real pixel
//! size, without a legacy raw-bitmap conversion — a well-supported convention since Windows
//! Vista (`.ico`) and macOS 10.7 (`.icns`). So generating either one is just "resize the
//! source PNG to each required size, PNG-encode each, wrap them in a small, hand-written
//! container" — no `ico`/`icns`-writing crate needed, just `image` for the resize/encode.

use std::io::{Cursor, Write};
use std::path::Path;

use image::imageops::FilterType;

fn resized_png_frames(source_png: &Path, sizes: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, String> {
    let image = image::open(source_png).map_err(|e| format!("reading {source_png:?}: {e}"))?;
    sizes
        .iter()
        .map(|&size| {
            let resized = image.resize_exact(size, size, FilterType::Lanczos3);
            let mut png_bytes = Vec::new();
            resized
                .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
                .map_err(|e| format!("encoding {size}x{size} frame: {e}"))?;
            Ok((size, png_bytes))
        })
        .collect()
}

/// Standard Windows icon sizes -- 16/32/48 for classic small contexts (title bar, Alt-Tab),
/// 128/256 for large-icon views in Explorer (all still real destinations for the .exe's own
/// resource, patched in-place by `rcedit` -- see `bundle.rs`).
const ICO_SIZES: [u32; 5] = [16, 32, 48, 128, 256];

/// Writes a Windows `.ico` containing `source_png` resized to each of `ICO_SIZES`, each
/// frame stored as its own PNG -- the ICONDIR/ICONDIRENTRY structure itself is the classic,
/// unchanged binary format (`bWidth`/`bHeight` of 0 conventionally means 256, per the spec).
pub fn generate_ico(source_png: &Path, dest_ico: &Path) -> Result<(), String> {
    let frames = resized_png_frames(source_png, &ICO_SIZES)?;

    let mut file = std::fs::File::create(dest_ico).map_err(|e| e.to_string())?;
    file.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // reserved
    file.write_all(&1u16.to_le_bytes()).map_err(|e| e.to_string())?; // type: icon
    file.write_all(&(frames.len() as u16).to_le_bytes()).map_err(|e| e.to_string())?;

    let mut offset = 6 + (frames.len() as u32) * 16; // header + one ICONDIRENTRY per frame
    for (size, png_bytes) in &frames {
        let size_byte = if *size >= 256 { 0u8 } else { *size as u8 };
        file.write_all(&[size_byte, size_byte, 0, 0]).map_err(|e| e.to_string())?; // w, h, palette, reserved
        file.write_all(&1u16.to_le_bytes()).map_err(|e| e.to_string())?; // color planes
        file.write_all(&32u16.to_le_bytes()).map_err(|e| e.to_string())?; // bits per pixel
        file.write_all(&(png_bytes.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&offset.to_le_bytes()).map_err(|e| e.to_string())?;
        offset += png_bytes.len() as u32;
    }
    for (_, png_bytes) in &frames {
        file.write_all(png_bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `(OSType code, pixel size)` -- a minimal but real modern icon family (128/256/512/1024px,
/// covering `ic07`/`ic08`/`ic09`/`ic10`). Not the full Apple-recommended set (this skips the
/// small 16/32/64px legacy sizes and the explicit `@2x` variants of each) -- good enough for
/// the Dock/Finder to show a correct, sharp icon at every size actually used there, which is
/// all this needs to solve; a more exhaustive set can be added later if a real gap shows up.
const ICNS_TYPES: [(&[u8; 4], u32); 4] = [(b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024)];

/// Writes a macOS `.icns` containing `source_png` resized to each of `ICNS_TYPES`'s sizes --
/// see `bundle.rs`'s own doc comment on where this replaces the shell's default
/// `Contents/Resources/servo.icns` (the exact filename `Info.plist`'s `CFBundleIconFile`
/// already references, so no `Info.plist` edit is needed, just overwriting that one file).
pub fn generate_icns(source_png: &Path, dest_icns: &Path) -> Result<(), String> {
    let sizes: Vec<u32> = ICNS_TYPES.iter().map(|&(_, size)| size).collect();
    let frames = resized_png_frames(source_png, &sizes)?;

    let mut body = Vec::new();
    for ((type_code, _), (_, png_bytes)) in ICNS_TYPES.iter().zip(frames.iter()) {
        body.extend_from_slice(*type_code);
        let entry_len = 8 + png_bytes.len() as u32; // type + length fields + payload
        body.extend_from_slice(&entry_len.to_be_bytes());
        body.extend_from_slice(png_bytes);
    }

    let mut file = std::fs::File::create(dest_icns).map_err(|e| e.to_string())?;
    file.write_all(b"icns").map_err(|e| e.to_string())?;
    let total_len = 8 + body.len() as u32; // "icns" + length field + body
    file.write_all(&total_len.to_be_bytes()).map_err(|e| e.to_string())?;
    file.write_all(&body).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png(dir: &Path, size: u32) -> std::path::PathBuf {
        let path = dir.join("icon.png");
        let image = image::DynamicImage::new_rgba8(size, size);
        image.save(&path).unwrap();
        path
    }

    #[test]
    fn ico_has_a_valid_header_and_one_directory_entry_per_size() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_test_png(dir.path(), 512);
        let dest = dir.path().join("out.ico");

        generate_ico(&source, &dest).unwrap();
        let bytes = std::fs::read(&dest).unwrap();

        assert_eq!(&bytes[0..2], &0u16.to_le_bytes(), "reserved field must be 0");
        assert_eq!(&bytes[2..4], &1u16.to_le_bytes(), "type field must be 1 (icon)");
        let count = u16::from_le_bytes([bytes[4], bytes[5]]);
        assert_eq!(count as usize, ICO_SIZES.len());
        // Every frame's embedded image starts with the real PNG signature -- confirms the
        // "store a real PNG, not a raw DIB" convention actually produced valid PNG data.
        for i in 0..count as usize {
            let entry = &bytes[6 + i * 16..6 + (i + 1) * 16];
            let data_size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let data_offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            let frame = &bytes[data_offset..data_offset + data_size];
            assert_eq!(&frame[0..8], b"\x89PNG\r\n\x1a\n", "frame {i} isn't a valid PNG");
        }
    }

    #[test]
    fn icns_has_a_valid_magic_and_length_matching_the_actual_file_size() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_test_png(dir.path(), 512);
        let dest = dir.path().join("out.icns");

        generate_icns(&source, &dest).unwrap();
        let bytes = std::fs::read(&dest).unwrap();

        assert_eq!(&bytes[0..4], b"icns");
        let declared_len = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
        assert_eq!(declared_len, bytes.len(), "declared length must match the real file size");

        // Walk the entries and confirm each one's own length field is internally consistent
        // and its payload is a real PNG.
        let mut offset = 8;
        let mut seen_types = Vec::new();
        while offset < bytes.len() {
            let type_code = &bytes[offset..offset + 4];
            let entry_len = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
            let payload = &bytes[offset + 8..offset + entry_len];
            assert_eq!(&payload[0..8], b"\x89PNG\r\n\x1a\n", "entry payload isn't a valid PNG");
            seen_types.push(String::from_utf8_lossy(type_code).into_owned());
            offset += entry_len;
        }
        assert_eq!(seen_types, vec!["ic07", "ic08", "ic09", "ic10"]);
    }
}
