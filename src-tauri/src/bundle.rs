//! Orchestrates a real release: download the prebuilt shell (`shell.rs`), place the user's
//! content into it (`packer.rs`), write `launch.json`, and zip the result — once per
//! selected platform. This is the real implementation behind the "Generate release" button;
//! see this project's own README/CLAUDE.md for why it never shells out to `mach` or requires
//! a Rust/Python toolchain on the machine running Packmaster itself.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::installer;
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
pub async fn check_shell_availability(platforms: Vec<String>, steam: bool) -> Vec<(String, bool)> {
    let mut results = Vec::with_capacity(platforms.len());
    for platform in platforms {
        let available = shell::is_shell_available(&platform, steam).await;
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

    // The union of "build a portable zip" and "build an installer" per platform — a
    // platform can be requested for either, both, or neither; either one means the same
    // shell download + content-packing work has to happen for it.
    let platforms: Vec<&'static str> = ["windows", "linux", "macos"]
        .into_iter()
        .filter(|&p| settings.portable.get(p) || settings.installers.get(p).enabled)
        .collect();
    if platforms.is_empty() {
        return Err("no platform selected".to_string());
    }

    let steam = settings.plugins.steam.enabled;
    let steam_app_id = settings.plugins.steam.app_id.trim().to_string();
    if steam && (steam_app_id.is_empty() || !steam_app_id.chars().all(|c| c.is_ascii_digit())) {
        return Err("Steam is enabled but the App ID isn't a valid positive number".to_string());
    }

    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())?
        .to_path_buf();
    let release_dir = exe_dir.join("release");
    std::fs::create_dir_all(&release_dir).map_err(|e| e.to_string())?;

    let window_title = (!name.trim().is_empty()).then(|| name.trim().to_string());
    // `package_name` (just the sanitized game name) is what installers use — WiX's
    // INSTALLDIR, the .deb's package name, the .dmg's volume name, etc. — while the
    // portable zip's own filename folds the version in too, since it isn't a package
    // identifier anywhere the way the installer name is.
    let package_name = sanitize_name(window_title.as_deref().unwrap_or("game"));
    let raw_version = version.trim().to_string();
    let zip_stem = file_stem(window_title.as_deref().unwrap_or("game"), &raw_version);

    for &platform in &platforms {
        emit_progress(&app, platform, "checking", 0.0);
        if !shell::is_shell_available(platform, steam).await {
            let variant = if steam { " Steam-enabled" } else { "" };
            return Err(format!(
                "no published{variant} shell release found for {platform} (targeting {})",
                shell::resolve_shell_version().await
            ));
        }
        let installer_settings = settings.installers.get(platform);
        if installer_settings.enabled {
            let (available, reason) = installer::check_installer_availability(platform);
            if !available {
                return Err(format!(
                    "can't build an installer for {platform}: {}",
                    reason.unwrap_or_else(|| "not available on this machine".to_string())
                ));
            }
        }

        let shell_root = shell::ensure_shell(&app, platform, steam, |fraction| {
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

        emit_progress(&app, platform, "packing", 0.4);
        let content_dest = content_root.join(packer::CONTENT_SUBDIR);
        packer::place_content(&content_dir, &content_dest, &settings.compression, window_title.clone())?;
        packer::write_launch_json(&binary_dir, settings.compression.enabled, window_title.as_deref())?;
        if steam {
            write_steam_appid(&binary_dir, &steam_app_id)?;
        }
        apply_icon(&app, &binary_dir, &content_dir, platform, &settings.icon).await?;

        if settings.portable.get(platform) {
            emit_progress(&app, platform, "zipping", 0.7);
            let zip_path = release_dir.join(format!("{zip_stem}_{platform}.zip"));
            if platform == "macos" {
                // macOS's own `.app` bundle is already the single, self-contained,
                // double-click-to-run thing a player needs -- Finder never shows what's
                // inside it. Wrapping it in another folder (as the shared `else` branch
                // below does for Windows/Linux, where staging_dir holds many loose
                // files that really do need one) only adds a pointless extra folder to
                // open first. Zip play.app's own contents directly under a
                // `<package_name>.app/` prefix instead -- same renamed-to-the-game's-
                // name treatment the other platforms already get, at the zip root.
                let app_dir = staging_dir.join("play.app");
                zip_dir_as(&app_dir, &format!("{package_name}.app"), &zip_path)?;
            } else {
                zip_dir_as(&staging_dir, &package_name, &zip_path)?;
            }
        }

        if installer_settings.enabled {
            for format in &installer_settings.formats {
                emit_progress(&app, platform, "packaging", 0.85);
                let result = match format.as_str() {
                    "msi" => installer::build_msi(&staging_dir, &release_dir, &package_name, &raw_version),
                    "dmg" => installer::build_dmg(&staging_dir, &release_dir, &package_name, &raw_version),
                    "deb" => installer::build_deb(&staging_dir, &release_dir, &package_name, &raw_version),
                    other => Err(format!("unknown installer format {other:?}")),
                };
                result.map_err(|e| format!("building .{format} for {platform}: {e}"))?;
            }
        }

        std::fs::remove_dir_all(&staging_dir).ok();
        emit_progress(&app, platform, "done", 1.0);
    }

    Ok(release_dir.to_string_lossy().into_owned())
}

fn process_id() -> u32 {
    std::process::id()
}

/// Writes `steam_appid.txt` next to the packaged game's executable — Valve's own convention
/// for `steamworks::Client::init()` (see the engine repo's `protocols/steam.rs`) to find the
/// App ID when testing outside the real Steam client. Steam itself sets this automatically
/// once the game is actually published and launched through it; this file only matters for
/// local/direct-launch testing, but is harmless to ship either way.
fn write_steam_appid(binary_dir: &Path, app_id: &str) -> Result<(), String> {
    std::fs::write(binary_dir.join("steam_appid.txt"), app_id).map_err(|e| e.to_string())
}

/// Applies the user's custom game icon, if any, exactly the same way and to the exact same
/// locations as the engine's own `mach bundle --icon-png`/`--icon-ico` (see
/// `python/servo/post_build_commands.py` in the engine repo, and its own
/// CUSTOMIZATIONS.md "Runtime + post-build game icon" entry) — `binary_dir` is this
/// platform's exact analogue of that Python code's `stage_dir`/`Contents/MacOS`/`lib_dir`.
/// `ports/servoshell/desktop/headed_window.rs`'s `runtime_window_icon_bytes` is what actually
/// reads the PNG back at launch; there's no Packmaster-specific runtime code to keep in sync.
///
/// Neither setting explicitly chosen? Auto-detects an `icon.png`/`icon.ico` sitting directly
/// in `content_dir` before falling back to Roves' own branding -- mirrors
/// `post_build_commands.py`'s own identical default (see that function's own comment for
/// why: many bundlers already emit one there for their own PWA manifest). An explicit
/// `settings.icon` path always wins over this.
async fn apply_icon(
    app: &AppHandle,
    binary_dir: &Path,
    content_dir: &Path,
    platform: &str,
    icon: &crate::settings::IconSettings,
) -> Result<(), String> {
    let png_path = icon.png_path.clone().or_else(|| {
        let candidate = content_dir.join("icon.png");
        candidate.is_file().then(|| candidate.to_string_lossy().into_owned())
    });
    let ico_path = icon.ico_path.clone().or_else(|| {
        let candidate = content_dir.join("icon.ico");
        candidate.is_file().then(|| candidate.to_string_lossy().into_owned())
    });

    if let Some(png_path) = &png_path {
        if platform == "macos" {
            // Silently skipped, mirroring the engine's own --icon-png warning-and-ignore on
            // macOS -- its Dock/app icon has no runtime override yet, so there's nothing
            // this could actually change there.
        } else {
            std::fs::copy(png_path, binary_dir.join("icon.png")).map_err(|e| e.to_string())?;
        }
    }
    if let Some(ico_path) = &ico_path {
        if platform == "windows" {
            let exe_path = binary_dir.join("play.exe");
            patch_windows_exe_icon(app, &exe_path, ico_path).await?;
        }
    }
    Ok(())
}

const RCEDIT_URL: &str = "https://github.com/electron/rcedit/releases/download/v2.0.0/rcedit-x64.exe";

/// Downloads (once; cached under this app's cache dir) `rcedit-x64.exe` and uses it to patch
/// `exe_path`'s own icon resource in place — see the engine repo's own
/// `python/servo/post_build_commands.py`, `_ensure_rcedit`/`_patch_windows_exe_icon`, which
/// this mirrors exactly (same tool, same pinned release, same reasoning: rcedit is what
/// Electron itself uses to give a prebuilt shell a different icon per app, no recompile
/// needed).
async fn patch_windows_exe_icon(app: &AppHandle, exe_path: &Path, ico_path: &str) -> Result<(), String> {
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("tools");
    let rcedit_path = cache_dir.join("rcedit-x64.exe");
    if !rcedit_path.exists() {
        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
        let response = reqwest::get(RCEDIT_URL).await.map_err(|e| format!("downloading rcedit: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("downloading rcedit: HTTP {}", response.status()));
        }
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        std::fs::write(&rcedit_path, &bytes).map_err(|e| e.to_string())?;
    }
    let status = std::process::Command::new(&rcedit_path)
        .arg(exe_path)
        .arg("--set-icon")
        .arg(ico_path)
        .status()
        .map_err(|e| format!("running rcedit: {e}"))?;
    if !status.success() {
        return Err(format!("rcedit exited with {status}"));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    // Regression test for a real complaint: the macOS portable zip used to wrap play.app in
    // an extra <game-name>/ folder (the right call for Windows/Linux, where staging_dir holds
    // many loose files -- but play.app is already the single, self-contained,
    // double-click-to-run thing a player needs on macOS, and Finder never shows what's inside
    // it). Confirms the zip now puts <game-name>.app directly at the archive root instead.
    #[test]
    fn macos_zip_has_no_extra_wrapper_folder_around_the_app_bundle() {
        let staging_dir = tempfile::tempdir().unwrap();
        let app_dir = staging_dir.path().join("play.app");
        let macos_dir = app_dir.join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos_dir).unwrap();
        std::fs::write(macos_dir.join("play"), b"fake binary").unwrap();

        let zip_path = staging_dir.path().join("out.zip");
        zip_dir_as(&app_dir, "My Game.app", &zip_path).unwrap();

        let file = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();

        assert_eq!(
            names,
            vec![
                "My Game.app/Contents/".to_string(),
                "My Game.app/Contents/MacOS/".to_string(),
                "My Game.app/Contents/MacOS/play".to_string(),
            ]
        );
    }
}
