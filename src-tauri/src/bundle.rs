//! Orchestrates a real release: download the prebuilt shell (`shell.rs`), place the user's
//! content into it (`packer.rs`), write `launch.json`, and zip the result — once per
//! selected platform. This is the real implementation behind the "Generate release" button;
//! see this project's own README/CLAUDE.md for why it never shells out to `mach` or requires
//! a Rust/Python toolchain on the machine running Packmaster itself.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::packer;
use crate::settings::PackmasterSettings;
use crate::shell;

#[derive(Clone, Serialize)]
struct BundleProgress {
    platform: String,
    phase: String,
    fraction: f64,
}

fn emit_progress(app: &AppHandle, platform: &str, phase: &str, fraction: f64) {
    let _ = app.emit(
        "bundle-progress",
        BundleProgress {
            platform: platform.to_string(),
            phase: phase.to_string(),
            fraction,
        },
    );
}

/// Real feasibility check ("is this actually distributable") for each requested platform —
/// confirms the targeted shell release genuinely has a published asset there, rather than
/// assuming it based on the host OS the way the old mock's platform-hiding logic did (real
/// portable bundling works for any of the 3 platforms from any host, since it's just file
/// assembly against a downloaded prebuilt binary, not a local compile).
#[tauri::command]
pub async fn check_shell_availability(platforms: Vec<String>) -> Vec<(String, bool)> {
    let mut results = Vec::with_capacity(platforms.len());
    for platform in platforms {
        let available = shell::is_shell_available(&platform).await;
        results.push((platform, available));
    }
    results
}

/// `name`/`version` come from the frontend's own "Release info" fields (configure.tsx) —
/// derived from package.json but user-editable, so they're the actual source of truth for
/// both the bundled window title and the generated filenames, not re-derived here.
#[tauri::command]
pub async fn generate_release(
    app: AppHandle,
    settings: PackmasterSettings,
    name: String,
    version: String,
) -> Result<String, String> {
    let source_dir = settings
        .source_dir
        .as_ref()
        .ok_or_else(|| "no source folder selected".to_string())?;
    let content_dir = PathBuf::from(source_dir);

    let platforms = settings.portable.selected();
    if platforms.is_empty() {
        return Err("no platform selected".to_string());
    }

    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())?
        .to_path_buf();
    let release_dir = exe_dir.join("release");
    std::fs::create_dir_all(&release_dir).map_err(|e| e.to_string())?;

    let window_title = (!name.trim().is_empty()).then(|| name.trim().to_string());
    let file_stem = file_stem(window_title.as_deref().unwrap_or("game"), version.trim());

    for &platform in &platforms {
        emit_progress(&app, platform, "checking", 0.0);
        if !shell::is_shell_available(platform).await {
            return Err(format!(
                "no published shell release found for {platform} (targeting {})",
                shell::TARGET_SHELL_VERSION
            ));
        }

        let shell_root = shell::ensure_shell(&app, platform, |fraction| {
            emit_progress(&app, platform, "downloading", fraction);
        })
        .await?;

        emit_progress(&app, platform, "assembling", 0.0);
        let staging_dir = std::env::temp_dir().join(format!("roves-packmaster-{platform}-{}", process_id()));
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir).map_err(|e| e.to_string())?;
        }
        copy_dir_recursive(&shell_root, &staging_dir)?;

        let (binary_dir, content_root) = match platform {
            "macos" => (
                staging_dir.join("play.app").join("Contents").join("MacOS"),
                staging_dir.join("play.app").join("Contents").join("Resources"),
            ),
            _ => (staging_dir.clone(), staging_dir.clone()),
        };

        emit_progress(&app, platform, "packing", 0.5);
        packer::place_content(&content_dir, &content_root, &settings.compression, window_title.clone())?;
        packer::write_launch_json(&binary_dir, settings.compression.enabled, window_title.as_deref())?;

        emit_progress(&app, platform, "zipping", 0.8);
        let zip_path = release_dir.join(format!("{file_stem}_{platform}.zip"));
        zip_dir_as(&staging_dir, &file_stem, &zip_path)?;

        std::fs::remove_dir_all(&staging_dir).ok();
        emit_progress(&app, platform, "done", 1.0);
    }

    Ok(release_dir.to_string_lossy().into_owned())
}

fn process_id() -> u32 {
    std::process::id()
}

fn sanitize_name(name: &str) -> String {
    // '.' is allowed too (on top of the usual alphanumeric/-/_) -- otherwise a plain
    // dotted version like "1.0.0" would come out as "1_0_0", which is a needless mangling
    // for a character every OS's filesystem already accepts fine.
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "game".to_string()
    } else {
        trimmed.to_lowercase()
    }
}

/// `<name>` or `<name>_<version>` — the base every generated zip is named after, e.g.
/// `mygame_1.0.0_windows.zip`. Version is folded in only when actually provided (blank is
/// common: a game with no package.json and a user who hasn't typed one in yet).
fn file_stem(name: &str, version: &str) -> String {
    let name = sanitize_name(name);
    if version.is_empty() {
        name
    } else {
        format!("{name}_{}", sanitize_name(version))
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Zips `src`'s contents under a single top-level `<folder_name>/` entry (not `src`'s own
/// contents at the archive root) — same reasoning as release.yml's own zip step: extracting
/// otherwise dumps every file straight into whatever folder you extract into.
fn zip_dir_as(src: &Path, folder_name: &str, zip_path: &Path) -> Result<(), String> {
    let file = std::fs::File::create(zip_path).map_err(|e| e.to_string())?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let archive_path = format!("{folder_name}/{}", rel.to_string_lossy().replace('\\', "/"));
        if entry.file_type().is_dir() {
            writer
                .add_directory(format!("{archive_path}/"), options)
                .map_err(|e| e.to_string())?;
        } else {
            writer.start_file(archive_path, options).map_err(|e| e.to_string())?;
            let bytes = std::fs::read(entry.path()).map_err(|e| e.to_string())?;
            writer.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }
    writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}
