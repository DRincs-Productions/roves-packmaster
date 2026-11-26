//! Orchestrates a real release: for each selected desktop platform, download the engine's
//! prebuilt CEF player package (`shell.rs`), add the user's build
//! output (`cef_player.rs`), and keep the result as the portable zip and/or wrap it into an
//! installer; then the mobile outputs. This is the real implementation behind the "Generate
//! release" button; see this project's own README/CLAUDE.md for why it never shells out to
//! `mach` or requires a Rust/Python toolchain on the machine running Packmaster itself.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::android;
use crate::cef_player::{self, Platform};
use crate::installer;
use crate::settings::PackmasterSettings;
use crate::shell;
use crate::signing;

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
/// confirms the targeted engine release genuinely has the matching player package published
/// (real portable bundling works for any of the 3 platforms from any host, since it's just
/// file assembly against a downloaded prebuilt package, not a local compile). A combination
/// the engine doesn't publish (Steam + MCP) counts as unavailable.
#[tauri::command]
pub async fn check_shell_availability(platforms: Vec<String>, steam: bool, mcp: bool) -> Vec<(String, bool)> {
    let mut results = Vec::with_capacity(platforms.len());
    for id in platforms {
        let available = match Platform::from_id(&id).map(|p| shell::asset_name(p, steam, mcp)) {
            Some(Ok(asset)) => shell::is_asset_available(&asset).await,
            _ => false,
        };
        results.push((id, available));
    }
    results
}

/// `name`/`version` come from the frontend's own "Release info" fields (configure.tsx) —
/// derived from package.json but user-editable, so they're the actual source of truth for
/// the generated filenames, not re-derived here.
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
    // download + assembly work has to happen for it.
    let platforms: Vec<&'static str> = ["windows", "linux", "macos"]
        .into_iter()
        .filter(|&p| settings.portable.get(p) || settings.installers.get(p).enabled)
        .collect();
    if platforms.is_empty() && !settings.mobile.android.enabled && !settings.mobile.ios.enabled {
        return Err("no platform selected".to_string());
    }

    let steam = settings.plugins.steam.enabled;
    let steam_app_id = settings.plugins.steam.app_id.trim().to_string();
    if steam && (steam_app_id.is_empty() || !steam_app_id.chars().all(|c| c.is_ascii_digit())) {
        return Err("Steam is enabled but the App ID isn't a valid positive number".to_string());
    }
    let mcp = settings.plugins.mcp.enabled;

    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())?
        .to_path_buf();
    let release_dir = exe_dir.join("release");
    std::fs::create_dir_all(&release_dir).map_err(|e| e.to_string())?;

    let window_title = (!name.trim().is_empty()).then(|| name.trim().to_string());
    // `package_name` (just the sanitized game name) is what installers use — WiX's
    // INSTALLDIR, the .deb's package name, the .dmg's volume name, etc. — and the top folder
    // (`<name>/`, or `<name>.app/` on macOS) of the portable zip, while the zip's own
    // filename folds the version in too.
    let package_name = sanitize_name(window_title.as_deref().unwrap_or("game"));
    let raw_version = version.trim().to_string();
    let zip_stem = file_stem(window_title.as_deref().unwrap_or("game"), &raw_version);

    for &id in &platforms {
        let platform = Platform::from_id(id).ok_or_else(|| format!("unknown platform {id}"))?;
        emit_progress(&app, id, "checking", 0.0);
        let asset = shell::asset_name(platform, steam, mcp)?;
        if !shell::is_asset_available(&asset).await {
            return Err(format!(
                "{asset} is not published in the targeted Roves release ({})",
                shell::resolve_shell_version().await
            ));
        }
        let installer_settings = settings.installers.get(id);
        if installer_settings.enabled {
            let (available, reason) = installer::check_installer_availability(id);
            if !available {
                return Err(format!(
                    "can't build an installer for {id}: {}",
                    reason.unwrap_or_else(|| "not available on this machine".to_string())
                ));
            }
        }

        let base_zip = shell::ensure_asset(&app, &asset, |fraction| {
            emit_progress(&app, id, "downloading", fraction);
        })
        .await?;

        emit_progress(&app, id, "assembling", 0.4);
        let work_dir = std::env::temp_dir().join(format!("roves-packmaster-{id}-{}", process_id()));
        if work_dir.exists() {
            std::fs::remove_dir_all(&work_dir).map_err(|e| e.to_string())?;
        }
        let result = build_desktop(
            &app,
            platform,
            &base_zip,
            &content_dir,
            &settings,
            &DesktopTarget {
                work_dir: &work_dir,
                release_dir: &release_dir,
                package_name: &package_name,
                zip_stem: &zip_stem,
                version: &raw_version,
                steam_app_id: steam.then_some(steam_app_id.as_str()),
                mcp,
            },
        );
        std::fs::remove_dir_all(&work_dir).ok();
        result?;
        emit_progress(&app, id, "done", 1.0);
    }

    if settings.mobile.android.enabled {
        emit_progress(&app, "android", "checking", 0.0);
        let (available, reason) = android::check_android_availability();
        if !available {
            return Err(reason.unwrap_or_else(|| "Android isn't available on this machine".to_string()));
        }

        // The explicit or auto-detected icon.png -- see `resolve_icon_path`.
        let icon_path = resolve_icon_path(&settings.icon, &content_dir);
        let signing_config = if settings.mobile.advanced.release_signing_enabled {
            match signing::load_signing_config(&app)? {
                Some(config) => Some(config),
                None => {
                    return Err(
                        "Release signing is enabled, but no keystore is configured yet -- generate or import one first."
                            .to_string(),
                    );
                },
            }
        } else {
            None
        };
        let options = android::AndroidBuildOptions {
            content_dir: &content_dir,
            icon_png: icon_path.as_deref().map(Path::new),
            app_name_override: &settings.mobile.advanced.app_name,
            orientation_override: &settings.mobile.advanced.orientation,
            signing: signing_config.as_ref(),
        };
        let apk_path = android::build_apk(&app, &options, |phase, fraction| {
            emit_progress(&app, "android", phase, fraction);
        })
        .await?;
        let dest = release_dir.join(format!("{zip_stem}_android.apk"));
        std::fs::copy(&apk_path, &dest).map_err(|e| e.to_string())?;
        emit_progress(&app, "android", "done", 1.0);
    }

    if settings.mobile.ios.enabled {
        emit_progress(&app, "ios", "checking", 0.0);
        let (available, reason) = crate::ios::check_ios_availability();
        if !available {
            return Err(reason.unwrap_or_else(|| "iOS isn't available on this machine".to_string()));
        }

        let ios_signing_config = if settings.mobile.ios_advanced.release_signing_enabled {
            match crate::ios_signing::load_ios_signing_config(&app)? {
                Some(config) => Some(config),
                None => {
                    return Err(
                        "Release signing is enabled, but no certificate/provisioning profile is configured yet -- import one first."
                            .to_string(),
                    );
                },
            }
        } else {
            None
        };
        let ios_options = crate::ios::IosBuildOptions {
            content_dir: &content_dir,
            app_name_override: &settings.mobile.ios_advanced.app_name,
            bundle_id_override: &settings.mobile.ios_advanced.bundle_id,
            signing: ios_signing_config.as_ref(),
        };
        let ios_output_path = crate::ios::build_ios(&app, &ios_options, |phase, fraction| {
            emit_progress(&app, "ios", phase, fraction);
        })
        .await?;
        // A signed build produces a single .ipa file; an unsigned build produces a
        // RovesGame.app *directory* (a Simulator app bundle isn't a single file) -- copy
        // accordingly rather than assuming either shape.
        if ios_output_path.is_dir() {
            let dest = release_dir.join(format!("{zip_stem}_ios.app"));
            if dest.exists() {
                std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
            }
            copy_dir_recursive(&ios_output_path, &dest)?;
        } else {
            let dest = release_dir.join(format!("{zip_stem}_ios.ipa"));
            std::fs::copy(&ios_output_path, &dest).map_err(|e| e.to_string())?;
        }
        emit_progress(&app, "ios", "done", 1.0);
    }

    Ok(release_dir.to_string_lossy().into_owned())
}

fn process_id() -> u32 {
    std::process::id()
}

/// Where and how one desktop platform's outputs are written.
struct DesktopTarget<'a> {
    /// Scratch directory, removed afterwards by the caller.
    work_dir: &'a Path,
    release_dir: &'a Path,
    package_name: &'a str,
    zip_stem: &'a str,
    version: &'a str,
    /// `Some` for Steam packages: written to `steam_appid.txt` next to the executable.
    steam_app_id: Option<&'a str>,
    /// Keep the player's local MCP bridge (the `_mcp` asset).
    mcp: bool,
}

/// `<stem>_<platform>.zip`; the MCP development package is `<stem>_<platform>_mcp.zip`, so it
/// is never mistaken for the one to ship.
fn portable_zip_name(zip_stem: &str, platform: Platform, mcp: bool) -> String {
    let suffix = if mcp { "_mcp" } else { "" };
    format!("{zip_stem}_{}{suffix}.zip", platform.id())
}

/// Top folder of the portable zip: `<name>/` for Windows/Linux (many loose files), and the
/// renamed `<name>.app/` bundle itself on macOS (already the single double-click-to-run
/// thing a player needs; Finder never shows what's inside it).
fn top_folder(platform: Platform, package_name: &str) -> String {
    match platform {
        Platform::Macos => format!("{package_name}.app"),
        _ => package_name.to_string(),
    }
}

fn build_desktop(
    app: &AppHandle,
    platform: Platform,
    base_zip: &Path,
    content_dir: &Path,
    settings: &PackmasterSettings,
    target: &DesktopTarget<'_>,
) -> Result<(), String> {
    let id = platform.id();
    let mut base = cef_player::Base::open(base_zip, platform, target.steam_app_id.is_some())?;
    let icon_dir = target.work_dir.join("icon");
    let fallback_ico = base.fallback_ico(&icon_dir)?;
    let icon = cef_player::resolve_project_icon(
        platform,
        settings.icon.path.as_deref().map(Path::new),
        content_dir,
        fallback_ico.as_deref(),
        &icon_dir,
    )?;
    for warning in &icon.warnings {
        eprintln!("project icon ({id}): {warning}");
    }

    let top = top_folder(platform, target.package_name);
    let portable = settings.portable.get(id);
    let out_zip = if portable {
        target.release_dir.join(portable_zip_name(target.zip_stem, platform, target.mcp))
    } else {
        target.work_dir.join("package.zip")
    };
    emit_progress(app, id, "zipping", 0.6);
    cef_player::assemble(
        &mut base,
        &cef_player::AssembleOptions {
            content_dir,
            top_folder: &top,
            enable_mcp: target.mcp,
            steam_app_id: target.steam_app_id,
            icon: &icon,
            work_dir: target.work_dir,
        },
        &out_zip,
        &cef_player::set_exe_icon,
    )?;

    let installer_settings = settings.installers.get(id);
    if installer_settings.enabled {
        // Installers run only on their own host OS (`check_installer_availability`), where
        // extracting the assembled zip restores its Unix modes and symlinks.
        let stage_root = target.work_dir.join("stage");
        extract_zip(&out_zip, &stage_root)?;
        let stage_dir = match platform {
            // hdiutil gets the folder holding `<name>.app` (plus its Applications link).
            Platform::Macos => stage_root,
            _ => stage_root.join(&top),
        };
        for format in &installer_settings.formats {
            emit_progress(app, id, "packaging", 0.85);
            let (release_dir, name, version) = (target.release_dir, target.package_name, target.version);
            let result = match format.as_str() {
                "msi" => installer::build_msi(&stage_dir, release_dir, name, version),
                "dmg" => installer::build_dmg(&stage_dir, release_dir, name, version),
                "deb" => installer::build_deb(&stage_dir, release_dir, name, version, &base.executable),
                other => Err(format!("unknown installer format {other:?}")),
            };
            result.map_err(|e| format!("building .{format} for {id}: {e}"))?;
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive
        .extract(dest_dir)
        .map_err(|e| format!("extracting {}: {e}", zip_path.display()))
}

/// The Android icon: an explicit `settings.icon.path` always wins, else auto-detect
/// `icon.png` sitting directly in `content_dir` -- many bundlers already emit one there for
/// their own PWA manifest (confirmed: `pixi-vn-react-template`'s own `dist/` has exactly
/// this). The desktop players follow `cef_player::resolve_project_icon` instead.
fn resolve_icon_path(icon: &crate::settings::IconSettings, content_dir: &Path) -> Option<String> {
    icon.path.clone().or_else(|| {
        let candidate = content_dir.join("icon.png");
        candidate.is_file().then(|| candidate.to_string_lossy().into_owned())
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_outputs_are_named_per_platform_and_mcp_variant() {
        assert_eq!(portable_zip_name("mygame_1.0.0", Platform::Windows, false), "mygame_1.0.0_windows.zip");
        assert_eq!(portable_zip_name("mygame_1.0.0", Platform::Windows, true), "mygame_1.0.0_windows_mcp.zip");
        assert_eq!(portable_zip_name("mygame", Platform::Linux, false), "mygame_linux.zip");
        assert_eq!(portable_zip_name("mygame", Platform::Macos, true), "mygame_macos_mcp.zip");
        // The macOS zip holds the renamed .app directly at its root, no extra wrapper folder.
        assert_eq!(top_folder(Platform::Macos, "my_game"), "my_game.app");
        assert_eq!(top_folder(Platform::Windows, "my_game"), "my_game");
    }

}
