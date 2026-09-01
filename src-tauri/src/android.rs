//! Real Android APK generation for Packmaster: downloads the raw compiled `libservoshell.so`
//! and the `support/android/apk/` Gradle project (both published by the engine's own
//! `.github/workflows/android.yml` to the rolling "test" GitHub Release — see that workflow's
//! own comment; Android isn't part of a real, tagged release yet, so there's nothing versioned
//! to pin to), plus a portable JRE (Eclipse Temurin) and the Android SDK/NDK components
//! Gradle/`ndk-build` need — all auto-downloaded and cached under this app's own cache dir,
//! exactly like `shell.rs` already does for the desktop shell, so a game developer using
//! Packmaster needs no Android Studio, no Rust toolchain, nothing preinstalled.
//!
//! Mirrors the engine's own `python/servo/post_build_commands.py`'s `_bundle_android` (content
//! injection into `servoapp/src/main/assets/www/`, launcher icon into `res/mipmap/`, app name/
//! orientation as Gradle project properties) — see that function's own doc comment for the
//! design this ports to Rust. Reading the game's own web app manifest for defaults
//! (`read_web_manifest` below) mirrors that same file's `_read_web_manifest`.
//!
//! **The NDK is required even though Packmaster never compiles Rust**: `support/android/apk`'s
//! own Gradle build doesn't use `externalNativeBuild` — it shells out to `ndk-build` (via
//! `servoview/build.gradle.kts`'s `ndkbuild<Variant>` task, driven by `jni/Android.mk`) purely
//! to copy the prebuilt `libservoshell.so` *and* the NDK's own `libc++_shared.so` into the
//! APK's `jniLibs/` — there's no way to produce a working APK without that step.
//!
//! **Linux/macOS only for now**: that same Gradle task hardcodes the Unix script name
//! (`getNdkDir() + "/ndk-build"`, never `ndk-build.cmd`) with no Windows fallback — a real,
//! upstream Gradle-script gap this repo hasn't patched yet (see TODO.md), not a
//! Packmaster-specific choice. `check_android_availability` reports this honestly instead of
//! attempting a build that's known to fail on Windows.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

// ── Versions / URLs ─────────────────────────────────────────────────────────────────────
//
// Every pin here follows the same precedent as `bundle.rs`'s own `RCEDIT_URL`: a specific,
// verified, working version -- expected to need bumping occasionally, not a live "latest"
// alias (Google/Adoptium's URL scheme doesn't offer stable aliases for these two -- Adoptium's
// own /v3/binary/latest/ API below is the one exception, genuinely self-updating).

/// Android SDK "command line tools only" build id (the `_latest` suffix is part of this
/// specific build's own filename, not a live pointer to whatever's newest -- verified working
/// as of 2026-09 for linux/mac_x86_64/mac_arm64; bump when Google ships a newer one).
const CMDLINE_TOOLS_BUILD: &str = "15859902";

/// Must match the engine's own `python/servo/platform/build_target.py`, which hard-requires
/// major version 28 specifically (checks `source.properties`, rejects anything else) --
/// unlike `.github/workflows/android.yml`'s own dynamic `sdkmanager --list` resolution, this
/// can't easily re-resolve "latest r28.x" without already having a JRE+sdkmanager bootstrapped
/// first, so it's a plain pinned version instead (r28c, the last r28 release before r29).
const NDK_VERSION: &str = "r28c";

const ANDROID_PLATFORM: &str = "37";
const BUILD_TOOLS_VERSION: &str = "36.0.0";
/// Feature (major) version only -- Adoptium's `latest` endpoint resolves the exact patch
/// build itself, so this never goes stale the way `CMDLINE_TOOLS_BUILD`/`NDK_VERSION` can.
const JRE_FEATURE_VERSION: &str = "21";

/// Where `.github/workflows/android.yml` (this repo -- DRincs-Productions/roves) publishes
/// `roves_android_native_arm64.zip`/`roves_android_project.zip`. Not `shell::TARGET_SHELL_VERSION`
/// (a real, tagged release) -- Android is still experimental (see README.md's "Supported
/// platforms" table), so the rolling "test" tag is the only channel that has these assets at
/// all today.
const ANDROID_TEST_RELEASE_TAG: &str = "test";
const ANDROID_REPO: &str = "DRincs-Productions/roves";

/// Only this ABI is published (see android.yml) -- virtually every real Android device.
const ARCH_STRING: &str = "Arm64"; // servoapp's own Gradle variant naming (assemble<Arch>Debug)
const RUST_TRIPLE: &str = "aarch64-linux-android";

// Same 3 filenames, same order, as the frontend's own `readWebManifest` and the engine's own
// `_read_web_manifest` -- kept in sync across all three independent implementations (Rust
// here, TypeScript in the frontend, Python in the engine) by convention, not shared code.
const WEB_MANIFEST_CANDIDATES: [&str; 3] = ["manifest.webmanifest", "manifest.json", "site.webmanifest"];

/// Mirrors the engine's own `_ANDROID_ORIENTATION_MAP` (post_build_commands.py) -- the 6
/// directional PWA orientation values map to Android's `android:screenOrientation` enum;
/// "any"/"natural"/anything else/missing all fall back to "unspecified" (OS/sensor decides).
fn android_orientation(pwa_value: &str) -> &'static str {
    match pwa_value {
        "landscape" => "sensorLandscape",
        "landscape-primary" => "landscape",
        "landscape-secondary" => "reverseLandscape",
        "portrait" => "sensorPortrait",
        "portrait-primary" => "portrait",
        "portrait-secondary" => "reversePortrait",
        _ => "unspecified",
    }
}

/// Real feasibility check -- see this module's own top comment for why Windows isn't
/// supported yet. `(available, reason_if_not)`, matching `installer::check_installer_availability`'s
/// own shape.
pub fn check_android_availability() -> (bool, Option<String>) {
    if cfg!(target_os = "windows") {
        (
            false,
            Some(
                "Android packaging isn't supported on Windows yet -- the engine's own Gradle \
                 build shells out to `ndk-build` without a Windows (`ndk-build.cmd`) fallback."
                    .to_string(),
            ),
        )
    } else {
        (true, None)
    }
}

#[derive(Debug, Default, Clone)]
pub struct WebManifestInfo {
    pub name: Option<String>,
    pub short_name: Option<String>,
    pub orientation: Option<String>,
}

/// Reads the game's own web app manifest directly inside `content_dir` -- mirrors the
/// frontend's `readWebManifest`/the engine's `_read_web_manifest`, see `WEB_MANIFEST_CANDIDATES`.
/// Returns `None` if none of the 3 candidates exist or parse as a JSON object.
pub fn read_web_manifest(content_dir: &Path) -> Option<WebManifestInfo> {
    for candidate in WEB_MANIFEST_CANDIDATES {
        let path = content_dir.join(candidate);
        if !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let Some(obj) = json.as_object() else { continue };
        return Some(WebManifestInfo {
            name: obj.get("name").and_then(|v| v.as_str()).map(str::to_string),
            short_name: obj.get("short_name").and_then(|v| v.as_str()).map(str::to_string),
            orientation: obj.get("orientation").and_then(|v| v.as_str()).map(str::to_string),
        });
    }
    None
}

fn tools_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("android-tools"))
}

// ── Generic download/extract helpers (mirrors shell.rs's own download_file/extract_zip) ───

async fn download_with_retries(
    url: &str,
    dest: &Path,
    on_progress: &mut impl FnMut(f64) + Send,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let result: Result<(), String> = async {
            let response =
                reqwest::get(url).await.map_err(|e| format!("downloading {url}: {e}"))?;
            if !response.status().is_success() {
                return Err(format!("downloading {url}: HTTP {}", response.status()));
            }
            let total = response.content_length().unwrap_or(0);
            let mut downloaded: u64 = 0;
            let mut file = tokio::fs::File::create(dest).await.map_err(|e| e.to_string())?;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                file.write_all(&chunk).await.map_err(|e| e.to_string())?;
                downloaded += chunk.len() as u64;
                if total > 0 {
                    on_progress(downloaded as f64 / total as f64);
                }
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                tokio::fs::remove_file(dest).await.ok();
                if attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                }
            },
        }
    }
    Err(format!("after {MAX_ATTEMPTS} attempts: {last_err}"))
}

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(dest_dir).map_err(|e| format!("extracting {zip_path:?}: {e}"))
}

fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest_dir).map_err(|e| format!("extracting {archive_path:?}: {e}"))
}

/// Many of these downloads extract to a single, version-named top-level folder (e.g.
/// `android-ndk-r28c/`, `jdk-21.0.12.1+1-jre/`) whose exact name isn't worth hardcoding --
/// this finds it instead of guessing.
fn find_single_subdir(parent: &Path) -> Result<PathBuf, String> {
    let mut dirs = std::fs::read_dir(parent)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path());
    let first = dirs.next().ok_or_else(|| format!("no subdirectory found under {parent:?}"))?;
    Ok(first)
}

// ── JRE (Eclipse Temurin, for Gradle itself) ────────────────────────────────────────────

fn adoptium_os_arch() -> Result<(&'static str, &'static str), String> {
    let os = if cfg!(target_os = "macos") {
        "mac"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("unsupported OS for the Android JRE".to_string());
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        return Err("unsupported CPU architecture for the Android JRE".to_string());
    };
    Ok((os, arch))
}

/// Downloads (once; cached) a portable Eclipse Temurin JRE and returns `JAVA_HOME` --
/// Gradle needs a JVM to run at all, and Packmaster's whole point is that a game developer
/// doesn't need one preinstalled (see `shell.rs`'s own doc comment on the same principle for
/// the engine shell itself).
pub async fn ensure_jre(app: &AppHandle, mut on_progress: impl FnMut(f64) + Send) -> Result<PathBuf, String> {
    let (os, arch) = adoptium_os_arch()?;
    let cache_dir = tools_cache_dir(app)?.join("jre").join(format!("{os}-{arch}"));
    let marker = cache_dir.join(".complete");
    if marker.exists() {
        on_progress(1.0);
        let extracted = find_single_subdir(&cache_dir)?;
        return Ok(if os == "mac" { extracted.join("Contents").join("Home") } else { extracted });
    }
    if cache_dir.exists() {
        tokio::fs::remove_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;

    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{JRE_FEATURE_VERSION}/ga/{os}/{arch}/jre/hotspot/normal/eclipse"
    );
    let archive_path = cache_dir.join("jre.tar.gz");
    download_with_retries(&url, &archive_path, &mut on_progress).await.map_err(|e| format!("downloading JRE: {e}"))?;
    extract_tar_gz(&archive_path, &cache_dir)?;
    tokio::fs::remove_file(&archive_path).await.ok();
    tokio::fs::write(&marker, b"1").await.map_err(|e| e.to_string())?;

    let extracted = find_single_subdir(&cache_dir)?;
    Ok(if os == "mac" { extracted.join("Contents").join("Home") } else { extracted })
}

// ── Android SDK (cmdline-tools + sdkmanager-installed components) ──────────────────────

fn sdk_os_tag() -> Result<&'static str, String> {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { Ok("mac_arm64") } else { Ok("mac_x86_64") }
    } else if cfg!(target_os = "linux") {
        Ok("linux")
    } else {
        Err("unsupported OS for the Android SDK".to_string())
    }
}

/// Downloads (once; cached) the Android SDK "command line tools" and uses `sdkmanager` to
/// install exactly the components `support/android/apk` needs (matching
/// `servoapp/build.gradle.kts`'s own `compileSdk`/`buildToolsVersion`), accepting every
/// license non-interactively -- same effect as `.github/workflows/android.yml`'s own
/// `yes | sdkmanager --licenses`, piped here instead of shelled through `yes`. Returns the
/// SDK root (`ANDROID_SDK_ROOT`).
pub async fn ensure_android_sdk(
    app: &AppHandle,
    java_home: &Path,
    mut on_progress: impl FnMut(f64) + Send,
) -> Result<PathBuf, String> {
    let sdk_root = tools_cache_dir(app)?.join("android-sdk");
    let marker = sdk_root.join(".packages-installed");
    if marker.exists() {
        on_progress(1.0);
        return Ok(sdk_root);
    }
    tokio::fs::create_dir_all(&sdk_root).await.map_err(|e| e.to_string())?;

    let cmdline_tools_dir = sdk_root.join("cmdline-tools").join("latest");
    if !cmdline_tools_dir.join("bin").is_dir() {
        let os_tag = sdk_os_tag()?;
        let url = format!(
            "https://dl.google.com/android/repository/commandlinetools-{os_tag}-{CMDLINE_TOOLS_BUILD}_latest.zip"
        );
        let zip_path = sdk_root.join("cmdline-tools.zip");
        download_with_retries(&url, &zip_path, &mut on_progress)
            .await
            .map_err(|e| format!("downloading Android SDK command-line tools: {e}"))?;
        // The zip's own top-level folder is literally named `cmdline-tools` -- sdkmanager
        // itself insists it then sit *inside* a `latest/` (or other version-named) folder,
        // one level deeper, or it refuses to run at all (see Google's own docs on this).
        let extract_tmp = sdk_root.join("cmdline-tools-extract-tmp");
        if extract_tmp.exists() {
            tokio::fs::remove_dir_all(&extract_tmp).await.map_err(|e| e.to_string())?;
        }
        extract_zip(&zip_path, &extract_tmp)?;
        tokio::fs::remove_file(&zip_path).await.ok();
        tokio::fs::create_dir_all(&sdk_root.join("cmdline-tools")).await.map_err(|e| e.to_string())?;
        tokio::fs::rename(extract_tmp.join("cmdline-tools"), &cmdline_tools_dir)
            .await
            .map_err(|e| e.to_string())?;
        tokio::fs::remove_dir_all(&extract_tmp).await.ok();
    }

    let sdkmanager = cmdline_tools_dir.join("bin").join("sdkmanager");
    accept_sdk_licenses(&sdkmanager, &sdk_root, java_home)?;
    run_sdkmanager(
        &sdkmanager,
        &sdk_root,
        java_home,
        &["platform-tools", &format!("platforms;android-{ANDROID_PLATFORM}"), &format!("build-tools;{BUILD_TOOLS_VERSION}")],
    )?;

    tokio::fs::write(&marker, b"1").await.map_err(|e| e.to_string())?;
    Ok(sdk_root)
}

/// `sdkmanager --licenses` prompts once per not-yet-accepted license, reading `y`/`d` from
/// stdin -- piping a generous number of `y\n` lines has the same effect as CI's own
/// `yes | sdkmanager --licenses` without depending on the separate `yes` binary existing.
fn accept_sdk_licenses(sdkmanager: &Path, sdk_root: &Path, java_home: &Path) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = std::process::Command::new(sdkmanager)
        .arg(format!("--sdk_root={}", sdk_root.display()))
        .arg("--licenses")
        .env("JAVA_HOME", java_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("running sdkmanager --licenses: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        // Comfortably more than the number of distinct SDK licenses that have ever existed.
        // `.take()`, not `.as_mut()`: `stdin` must actually be dropped (closing the pipe) once
        // written, or sdkmanager blocks forever waiting for EOF after its last prompt instead
        // of exiting -- same reason `yes | sdkmanager --licenses` relies on `yes` eventually
        // getting SIGPIPE'd rather than sdkmanager reading from an indefinitely-open pipe.
        let _ = stdin.write_all("y\n".repeat(32).as_bytes());
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("sdkmanager --licenses exited with {status}"));
    }
    Ok(())
}

fn run_sdkmanager(sdkmanager: &Path, sdk_root: &Path, java_home: &Path, packages: &[&str]) -> Result<(), String> {
    let mut command = std::process::Command::new(sdkmanager);
    command.arg(format!("--sdk_root={}", sdk_root.display())).env("JAVA_HOME", java_home);
    for package in packages {
        command.arg(package);
    }
    let status = command.status().map_err(|e| format!("running sdkmanager: {e}"))?;
    if !status.success() {
        return Err(format!("sdkmanager exited with {status} while installing {packages:?}"));
    }
    Ok(())
}

// ── Android NDK (needed for ndk-build's jniLibs/libc++_shared.so packaging step) ────────

/// `(download_url, is_dmg)` -- macOS ships the NDK as a `.dmg` disk image, Linux/Windows as a
/// plain `.zip` (confirmed against the real, current download pages; Windows isn't reachable
/// here at all per `check_android_availability`, but the URL is still real).
fn ndk_download_info() -> Result<(String, bool), String> {
    if cfg!(target_os = "macos") {
        Ok((format!("https://dl.google.com/android/repository/android-ndk-{NDK_VERSION}-darwin.dmg"), true))
    } else if cfg!(target_os = "linux") {
        Ok((format!("https://dl.google.com/android/repository/android-ndk-{NDK_VERSION}-linux.zip"), false))
    } else {
        Err("unsupported OS for the Android NDK".to_string())
    }
}

/// Downloads (once; cached) NDK r28c and returns `ANDROID_NDK_ROOT`.
pub async fn ensure_ndk(app: &AppHandle, mut on_progress: impl FnMut(f64) + Send) -> Result<PathBuf, String> {
    let cache_dir = tools_cache_dir(app)?.join("ndk").join(NDK_VERSION);
    let marker = cache_dir.join(".complete");
    if marker.exists() {
        on_progress(1.0);
        return find_single_subdir(&cache_dir);
    }
    if cache_dir.exists() {
        tokio::fs::remove_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;

    let (url, is_dmg) = ndk_download_info()?;
    if is_dmg {
        let dmg_path = cache_dir.join("ndk.dmg");
        download_with_retries(&url, &dmg_path, &mut on_progress).await.map_err(|e| format!("downloading NDK: {e}"))?;
        extract_dmg(&dmg_path, &cache_dir)?;
        tokio::fs::remove_file(&dmg_path).await.ok();
    } else {
        let zip_path = cache_dir.join("ndk.zip");
        download_with_retries(&url, &zip_path, &mut on_progress).await.map_err(|e| format!("downloading NDK: {e}"))?;
        extract_zip(&zip_path, &cache_dir)?;
        tokio::fs::remove_file(&zip_path).await.ok();
    }
    tokio::fs::write(&marker, b"1").await.map_err(|e| e.to_string())?;
    find_single_subdir(&cache_dir)
}

/// Mounts `dmg_path` via `hdiutil` (macOS only -- the NDK's `.dmg` packaging), copies the
/// NDK folder it contains out to `dest_dir`, then unmounts. `hdiutil` is a stock macOS tool,
/// no download needed.
fn extract_dmg(dmg_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let mount_point = std::env::temp_dir().join(format!("roves-packmaster-ndk-mount-{}", std::process::id()));
    std::fs::create_dir_all(&mount_point).map_err(|e| e.to_string())?;

    let attach_status = std::process::Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-mountpoint"])
        .arg(&mount_point)
        .arg(dmg_path)
        .status()
        .map_err(|e| format!("running hdiutil attach: {e}"))?;
    if !attach_status.success() {
        return Err(format!("hdiutil attach exited with {attach_status}"));
    }

    let copy_result = (|| -> Result<(), String> {
        let ndk_dir = find_single_subdir(&mount_point)?;
        copy_dir_recursive(&ndk_dir, dest_dir)
    })();

    let _ = std::process::Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mount_point).status();
    std::fs::remove_dir_all(&mount_point).ok();

    copy_result
}

// ── Android project + prebuilt native library (from the engine's rolling "test" release) ─

fn test_release_asset_url(asset_name: &str) -> String {
    format!("https://github.com/{ANDROID_REPO}/releases/download/{ANDROID_TEST_RELEASE_TAG}/{asset_name}")
}

/// Downloads (once per app run -- see the "always re-check" note below; not cached across
/// runs the way the JRE/SDK/NDK are) the engine's `support/android/apk/` Gradle project.
/// Unlike the JRE/SDK/NDK toolchains (pinned/self-updating, safe to cache indefinitely), this
/// tracks a rolling release that's overwritten on every push to the engine's `main` branch --
/// caching it would silently keep building against a stale snapshot of the engine's own
/// Android integration.
async fn download_android_project(app: &AppHandle, mut on_progress: impl FnMut(f64) + Send) -> Result<PathBuf, String> {
    let dest_dir = tools_cache_dir(app)?.join("android-project");
    if dest_dir.exists() {
        tokio::fs::remove_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;

    let zip_path = dest_dir.join("project.zip");
    download_with_retries(&test_release_asset_url("roves_android_project.zip"), &zip_path, &mut on_progress)
        .await
        .map_err(|e| format!("downloading the Android project: {e}"))?;
    extract_zip(&zip_path, &dest_dir)?;
    tokio::fs::remove_file(&zip_path).await.ok();
    // The zip's own root is `support/android/apk/...` (see android.yml's `zip -rq ...
    // support/android/apk`) -- already the exact relative layout `getTargetDir`'s "three
    // directories up from the Gradle project" assumption needs (see this module's own
    // `build_apk` doc comment), as long as `dest_dir` plays the role of the engine repo root.
    Ok(dest_dir)
}

async fn download_native_library(app: &AppHandle, mut on_progress: impl FnMut(f64) + Send) -> Result<PathBuf, String> {
    let dest_dir = tools_cache_dir(app)?.join("android-native");
    if dest_dir.exists() {
        tokio::fs::remove_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;

    let zip_path = dest_dir.join("native.zip");
    download_with_retries(&test_release_asset_url("roves_android_native_arm64.zip"), &zip_path, &mut on_progress)
        .await
        .map_err(|e| format!("downloading the compiled engine library: {e}"))?;
    extract_zip(&zip_path, &dest_dir)?;
    tokio::fs::remove_file(&zip_path).await.ok();
    let so_path = dest_dir.join("libservoshell.so");
    if !so_path.is_file() {
        return Err("downloaded native library zip didn't contain libservoshell.so".to_string());
    }
    Ok(so_path)
}

// ── Orchestration ────────────────────────────────────────────────────────────────────────

pub struct AndroidBuildOptions<'a> {
    pub content_dir: &'a Path,
    pub icon_png: Option<&'a Path>,
    pub app_name_override: &'a str,
    pub orientation_override: &'a str,
}

/// Builds a debug `.apk` and returns its path. `on_progress(phase, fraction)` mirrors
/// `bundle.rs`'s own per-platform progress events, with `"android"` as the pseudo-platform
/// name -- see that module's `emit_progress`.
///
/// Scratch layout note: `support/android/apk/servoview/build.gradle.kts`'s own
/// `getTargetDir`/`getJniLibsPath` (buildSrc/Interop.kt) hardcode "go up 3 directories from
/// the Gradle project root" to find where `target/<triple>/<debug|release>/jniLibs/` should
/// live -- i.e. they assume the engine repo's own layout (`support/android/apk/servoview/../
/// ../../` = repo root). This build copies the downloaded project into a scratch dir
/// preserving that exact `support/android/apk/` nesting, so that assumption still resolves
/// correctly to the scratch root instead of somewhere unwritable.
pub async fn build_apk(
    app: &AppHandle,
    options: &AndroidBuildOptions<'_>,
    mut on_progress: impl FnMut(&str, f64) + Send,
) -> Result<PathBuf, String> {
    let (available, reason) = check_android_availability();
    if !available {
        return Err(reason.unwrap_or_else(|| "Android packaging isn't available on this machine".to_string()));
    }

    on_progress("downloading-jre", 0.0);
    let java_home = ensure_jre(app, |f| on_progress("downloading-jre", f)).await?;

    on_progress("downloading-sdk", 0.0);
    let sdk_root = ensure_android_sdk(app, &java_home, |f| on_progress("downloading-sdk", f)).await?;

    on_progress("downloading-ndk", 0.0);
    let ndk_root = ensure_ndk(app, |f| on_progress("downloading-ndk", f)).await?;

    on_progress("downloading-project", 0.0);
    let project_root = download_android_project(app, |f| on_progress("downloading-project", f)).await?;

    on_progress("downloading-native", 0.0);
    let native_so = download_native_library(app, |f| on_progress("downloading-native", f)).await?;

    on_progress("assembling", 0.0);
    let scratch_root = std::env::temp_dir().join(format!("roves-packmaster-android-{}", std::process::id()));
    if scratch_root.exists() {
        std::fs::remove_dir_all(&scratch_root).map_err(|e| e.to_string())?;
    }
    // `project_root` already has the `support/android/apk/...` layout (see
    // `download_android_project`'s own comment) -- moving it wholesale into place is enough,
    // no need to know or reconstruct the individual module paths inside it.
    copy_dir_recursive(&project_root, &scratch_root)?;
    let apk_project_dir = scratch_root.join("support").join("android").join("apk");
    if !apk_project_dir.is_dir() {
        return Err(format!("downloaded Android project has no support/android/apk/ under {scratch_root:?}"));
    }

    // The raw compiled library, at the exact path `ndk-build`'s own `jni/Android.mk`
    // (`LOCAL_PATH := $(SERVO_TARGET_DIR)`) will look for it -- mirrors the engine's own
    // `post_build_commands.py` setting `env["SERVO_TARGET_DIR"] = path.dirname(servo_binary)`.
    let native_target_dir = scratch_root.join("native-src").join("debug");
    std::fs::create_dir_all(&native_target_dir).map_err(|e| e.to_string())?;
    std::fs::copy(&native_so, native_target_dir.join("libservoshell.so")).map_err(|e| e.to_string())?;

    // Content: mirrors `_bundle_android`'s unconditional `shutil.copytree(content_dir,
    // assets_dir)` -- no compression option exists for Android today (MainActivity.kt loads
    // a plain `file:///android_asset/www/index.html`, not a packed archive).
    let assets_dir = apk_project_dir.join("servoapp").join("src").join("main").join("assets").join("www");
    if assets_dir.exists() {
        std::fs::remove_dir_all(&assets_dir).map_err(|e| e.to_string())?;
    }
    copy_dir_recursive(options.content_dir, &assets_dir)?;

    // Icon: same replace-not-add-alongside reasoning as `_bundle_android` -- `@mipmap/servo`
    // resolves by resource name regardless of file extension, so the tracked `servo.webp`
    // must be removed first or Gradle sees a duplicate resource name.
    if let Some(icon_png) = options.icon_png {
        let mipmap_dir = apk_project_dir.join("servoapp").join("src").join("main").join("res").join("mipmap");
        for entry in std::fs::read_dir(&mipmap_dir).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            if path.file_stem().and_then(|s| s.to_str()) == Some("servo") {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
        std::fs::copy(icon_png, mipmap_dir.join("servo.png")).map_err(|e| e.to_string())?;
    }

    // App name / orientation: non-empty explicit override always wins, else fall back to the
    // game's own web app manifest, else Roves' own defaults -- same precedence as the
    // engine's own `mach bundle --android-app-name`/`--android-orientation` flags.
    let manifest = read_web_manifest(options.content_dir);
    let app_name = if !options.app_name_override.trim().is_empty() {
        options.app_name_override.trim().to_string()
    } else {
        manifest
            .as_ref()
            .and_then(|m| m.short_name.clone().or_else(|| m.name.clone()))
            .unwrap_or_else(|| "@string/app_name".to_string())
    };
    let orientation_pwa_value = if !options.orientation_override.trim().is_empty() {
        options.orientation_override.trim().to_string()
    } else {
        manifest.as_ref().and_then(|m| m.orientation.clone()).unwrap_or_default()
    };
    let orientation = android_orientation(&orientation_pwa_value);

    let gradlew = apk_project_dir.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&gradlew) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o111);
            let _ = std::fs::set_permissions(&gradlew, permissions);
        }
    }

    let task = format!(":servoapp:assemble{ARCH_STRING}Debug");
    let status = std::process::Command::new(&gradlew)
        .current_dir(&apk_project_dir)
        .arg("--no-daemon")
        .arg(&task)
        .arg(format!("-PservoScreenOrientation={orientation}"))
        .arg(format!("-PservoAppName={app_name}"))
        .env("JAVA_HOME", &java_home)
        .env("ANDROID_SDK_ROOT", &sdk_root)
        .env("ANDROID_NDK_ROOT", &ndk_root)
        .env("SERVO_TARGET_DIR", &native_target_dir)
        .status()
        .map_err(|e| format!("running gradlew: {e}"))?;
    if !status.success() {
        return Err(format!("gradlew exited with {status} while running {task}"));
    }

    // Same location `servoapp/build.gradle.kts`'s own `copyAndRename<Variant>APK` task
    // writes to -- `getTargetDir(debug=true, "arm64")`, i.e. `<scratch_root>/target/
    // aarch64-linux-android/debug/servoapp.apk` (see this function's own doc comment on the
    // "three directories up" assumption that makes `<scratch_root>` play the repo-root role).
    let apk_path = scratch_root
        .join("target")
        .join(RUST_TRIPLE)
        .join("debug")
        .join("servoapp.apk");
    if !apk_path.is_file() {
        return Err(format!("gradlew succeeded but no .apk found at the expected path {apk_path:?}"));
    }
    on_progress("done", 1.0);
    Ok(apk_path)
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
