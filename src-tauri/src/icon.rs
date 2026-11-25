//! Derives the Windows `.ico` (patched into `play.exe`) and the macOS `.icns` (the `.app`'s
//! bundle icon) from one square PNG, the project icon `cef_player::resolve_project_icon`
//! picked. Both containers come from maintained encoders, nothing is hand-written: `image`'s
//! `IcoEncoder` (PNG-encoded frames) and the `icns` crate (the same element types `iconutil`
//! writes for an `.iconset`). Neither depends on a host tool, so Packmaster produces both on
//! Windows, macOS and Linux alike.

use std::path::Path;

use image::imageops::FilterType;

fn open_rgba(source_png: &Path) -> Result<image::DynamicImage, String> {
    image::open(source_png).map_err(|e| format!("reading {}: {e}", source_png.display()))
}

fn resized_rgba(image: &image::DynamicImage, size: u32) -> Vec<u8> {
    let resized = if image.width() == size && image.height() == size {
        image.to_rgba8()
    } else {
        image.resize_exact(size, size, FilterType::Lanczos3).to_rgba8()
    };
    resized.into_raw()
}

/// Writes a `.ico` with one PNG-encoded frame per entry of `sizes` (1..=256 px each).
pub fn generate_ico_with_sizes(source_png: &Path, dest_ico: &Path, sizes: &[u32]) -> Result<(), String> {
    if sizes.is_empty() {
        return Err("no icon size to generate".to_string());
    }
    let image = open_rgba(source_png)?;
    let pixels: Vec<(u32, Vec<u8>)> = sizes.iter().map(|&size| (size, resized_rgba(&image, size))).collect();
    let frames = pixels
        .iter()
        .map(|(size, rgba)| {
            image::codecs::ico::IcoFrame::as_png(rgba, *size, *size, image::ExtendedColorType::Rgba8)
                .map_err(|e| format!("encoding the {size}x{size} frame: {e}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let file = std::fs::File::create(dest_ico).map_err(|e| format!("{}: {e}", dest_ico.display()))?;
    image::codecs::ico::IcoEncoder::new(std::io::BufWriter::new(file))
        .encode_images(&frames)
        .map_err(|e| format!("writing {}: {e}", dest_ico.display()))
}

/// The element types of an `iconutil` `.iconset` (16, 32, 128, 256 and 512 pt, each at 1x and
/// 2x), as `(type, pixel edge)`.
const ICNS_TYPES: [(icns::IconType, u32); 10] = [
    (icns::IconType::RGBA32_16x16, 16),
    (icns::IconType::RGBA32_16x16_2x, 32),
    (icns::IconType::RGBA32_32x32, 32),
    (icns::IconType::RGBA32_32x32_2x, 64),
    (icns::IconType::RGBA32_128x128, 128),
    (icns::IconType::RGBA32_128x128_2x, 256),
    (icns::IconType::RGBA32_256x256, 256),
    (icns::IconType::RGBA32_256x256_2x, 512),
    (icns::IconType::RGBA32_512x512, 512),
    (icns::IconType::RGBA32_512x512_2x, 1024),
];

/// Writes a `.icns` from a square PNG. Like the engine's `packaging/icns.sh`, sizes above the
/// source edge are left out rather than upscaled (a 16 px source still gets its 16 px element).
pub fn generate_icns(source_png: &Path, dest_icns: &Path) -> Result<(), String> {
    let image = open_rgba(source_png)?;
    let edge = image.width().min(image.height());
    let mut family = icns::IconFamily::new();
    for (icon_type, size) in ICNS_TYPES.into_iter().filter(|&(_, size)| size <= edge) {
        let element = icns::Image::from_data(icns::PixelFormat::RGBA, size, size, resized_rgba(&image, size))
            .map_err(|e| format!("building the {size}x{size} element: {e}"))?;
        family
            .add_icon_with_type(&element, icon_type)
            .map_err(|e| format!("encoding the {size}x{size} element: {e}"))?;
    }
    if family.is_empty() {
        return Err(format!("{} is smaller than 16x16", source_png.display()));
    }
    let file = std::fs::File::create(dest_icns).map_err(|e| format!("{}: {e}", dest_icns.display()))?;
    family
        .write(std::io::BufWriter::new(file))
        .map_err(|e| format!("writing {}: {e}", dest_icns.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png(dir: &Path, size: u32) -> std::path::PathBuf {
        let path = dir.join("icon.png");
        image::RgbaImage::from_pixel(size, size, image::Rgba([200, 30, 30, 255]))
            .save(&path)
            .unwrap();
        path
    }

    #[test]
    fn ico_has_one_decodable_frame_per_size() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_test_png(dir.path(), 512);
        let dest = dir.path().join("out.ico");
        generate_ico_with_sizes(&source, &dest, &[16, 32, 256]).unwrap();
        let bytes = std::fs::read(&dest).unwrap();
        assert_eq!(&bytes[0..4], &[0, 0, 1, 0]);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 3);
        // The decoder picks the largest frame.
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Ico).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 256));
    }

    #[test]
    fn icns_has_every_iconset_element_up_to_the_source_edge() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_test_png(dir.path(), 1024);
        let dest = dir.path().join("out.icns");
        generate_icns(&source, &dest).unwrap();
        let family = icns::IconFamily::read(std::fs::File::open(&dest).unwrap()).unwrap();
        let mut types = family.available_icons();
        types.sort_by_key(|t| (t.pixel_width(), t.pixel_density()));
        assert_eq!(types.len(), ICNS_TYPES.len());
        let big = family.get_icon_with_type(icns::IconType::RGBA32_512x512_2x).unwrap();
        assert_eq!((big.width(), big.height()), (1024, 1024));
        assert_eq!(&big.data()[..4], &[200, 30, 30, 255]);
    }

    #[test]
    fn icns_never_upscales_a_small_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = make_test_png(dir.path(), 48);
        let dest = dir.path().join("out.icns");
        generate_icns(&source, &dest).unwrap();
        let family = icns::IconFamily::read(std::fs::File::open(&dest).unwrap()).unwrap();
        assert!(family.available_icons().iter().all(|t| t.pixel_width() <= 48));
        assert!(family.has_icon_with_type(icns::IconType::RGBA32_16x16));
        assert!(family.has_icon_with_type(icns::IconType::RGBA32_16x16_2x));
        assert!(!family.has_icon_with_type(icns::IconType::RGBA32_128x128));
    }
}
