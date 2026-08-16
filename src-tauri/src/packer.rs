//! Places a game's built content into an assembled bundle — either packed (via the engine
//! repo's own `roves-content-packer` crate, linked in directly as a library so packing works
//! without shelling out to a separately-built tool or requiring a Rust toolchain on a game
//! developer's machine) or copied as loose files, matching `mach bundle`'s own
//! `--content-compress none` behavior. Mirrors `python/servo/post_build_commands.py`'s
//! `_place_bundle_content`/`_write_launch_config` — see that file for the Python original
//! this reimplements in Rust. Unlike that Python original, the window title/manifest name
//! isn't derived here (see `_resolve_window_title`) — `configure.tsx`'s "Release info" fields
//! own that job on the frontend now, since they're user-editable there.

use std::fs;
use std::path::Path;

use roves_content_packer::pack::{PackOptions, pack};
use roves_content_packer::size::parse_size;

use crate::settings::CompressionSettings;

/// Always the whole entry html file itself, in `content_dir`'s own root — mirrors the same
/// assumption Packmaster's source-selection screen already makes (`src/routes/index.tsx`
/// checks for exactly this file before letting a folder be picked at all).
pub const HTML_FILE: &str = "index.html";

/// Packs (or plain-copies) `content_dir` into `dest` — always `dest` itself, no nested
/// subfolder, so the same relative path ("" when packed, `HTML_FILE` when not) works
/// identically across Windows/Linux (content flat next to the binary) and macOS (content in
/// `Contents/Resources/`, see `bundle_launch.rs`'s `content_root`).
pub fn place_content(
    content_dir: &Path,
    dest: &Path,
    compression: &CompressionSettings,
    name: Option<String>,
) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    if !compression.enabled {
        copy_dir_recursive(content_dir, dest)?;
        return Ok(());
    }

    let exclude = compression
        .exclude
        .iter()
        .map(|p| glob::Pattern::new(p).map_err(|e| format!("invalid exclude glob {p:?}: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    let boot_include = compression
        .boot_include
        .iter()
        .map(|p| glob::Pattern::new(p).map_err(|e| format!("invalid boot-include glob {p:?}: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    let max_pack_size = parse_size(&compression.max_pack_size)?;

    pack(&PackOptions {
        input: content_dir.to_path_buf(),
        output: dest.to_path_buf(),
        level: compression.level,
        max_pack_size,
        exclude,
        html_file: HTML_FILE.to_string(),
        boot_include,
        name,
    })
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Written next to the shipped binary (`Contents/MacOS/` on macOS) — read back by
/// `ports/servoshell/desktop/bundle_launch.rs` on a plain double-click launch (no argv) to
/// resolve what to actually open. See that file's own doc comment for the exact schema this
/// mirrors.
pub fn write_launch_json(binary_dir: &Path, packed: bool, window_title: Option<&str>) -> Result<(), String> {
    let mut args: Vec<String> = vec!["--window-size".to_string(), "1280x720".to_string()];
    if let Some(title) = window_title {
        args.push("--window-title".to_string());
        args.push(title.to_string());
    }

    let mut launch = serde_json::Map::new();
    if packed {
        // "" -- content lives directly at content_root, no subfolder (see place_content).
        launch.insert("content_dir".to_string(), serde_json::Value::String(String::new()));
    } else {
        launch.insert("url".to_string(), serde_json::Value::String(HTML_FILE.to_string()));
    }
    launch.insert("args".to_string(), serde_json::json!(args));

    let text = serde_json::to_vec_pretty(&serde_json::Value::Object(launch)).map_err(|e| e.to_string())?;
    fs::write(binary_dir.join("launch.json"), text).map_err(|e| e.to_string())
}
