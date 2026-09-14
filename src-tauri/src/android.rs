//! Real Android APK generation for Packmaster: downloads the `support/android/apk/` Gradle
//! project (published by the engine's own `.github/workflows/android.yml` to the rolling
//! "test" GitHub Release — see that workflow's own comment; Android isn't part of a real,
//! tagged release yet, so there's nothing versioned to pin to), plus a portable JDK (Eclipse
//! Temurin) and the Android SDK components Gradle needs — all auto-downloaded and cached
//! under this app's own cache dir, exactly like `shell.rs` already does for the desktop
//! shell, so a game developer using Packmaster needs no Android Studio, no Rust toolchain,
//! nothing preinstalled.
//!
//! Mirrors the engine's own `python/servo/post_build_commands.py`'s `_bundle_android` (content
//! injection into `servoapp/src/main/assets/www/`, launcher icon into `res/mipmap/`, app name/
//! orientation as Gradle project properties) — see that function's own doc comment for the
//! design this ports to Rust. Reading the game's own web app manifest for defaults
//! (`read_web_manifest` below) mirrors that same file's `_read_web_manifest`.
//!
//! **2026-09-13: migrated to the engine's native-WebView Android port.** As of the engine's
//! own mobile pivot (Servo -> Android WebView/iOS WKWebView, see the engine repo's own
//! CUSTOMIZATIONS.md, "Mobile pivots from Servo to native WebView"), the game runs on
//! Android's own system `WebView` — no Servo, no JNI, no compiled `libservoshell.so` to
//! download, strip, or place, and therefore **no NDK at all**. This is why the whole previous
//! NDK/native-library-download/`llvm-strip` machinery that used to live in this file is gone:
//! `support/android/apk`'s own Gradle build no longer shells out to `ndk-build` at all (see
//! that project's own `servoapp/build.gradle.kts`), so there's nothing native for Packmaster
//! to compile, download, or fit into the APK anymore. This also means the earlier "Windows:
//! unblocked, not yet verified" NDK/`ndk-build.cmd` caveat that used to live here no longer
//! applies — there's no `ndk-build` invocation left to have a Windows-specific gap in.

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

const ANDROID_PLATFORM: &str = "37";
const BUILD_TOOLS_VERSION: &str = "36.0.0";
/// Feature (major) version only -- Adoptium's `latest` endpoint resolves the exact patch
/// build itself, so this never goes stale the way `CMDLINE_TOOLS_BUILD` can.
const JRE_FEATURE_VERSION: &str = "21";

/// Where `.github/workflows/android.yml` (this repo -- DRincs-Productions/roves) publishes
/// `roves_android_project.zip`. Not `shell::TARGET_SHELL_VERSION` (a real, tagged release) --
/// Android is still experimental (see README.md's "Supported platforms" table), so the
/// rolling "test" tag is the only channel that has this asset at all today.
const ANDROID_TEST_RELEASE_TAG: &str = "test";
const ANDROID_REPO: &str = "DRincs-Productions/roves";

/// Only this ABI is published (see android.yml) -- virtually every real Android device.
const ARCH_STRING: &str = "Arm64"; // servoapp's own Gradle variant naming (assemble<Arch>Debug)

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

/// Real feasibility check -- see this module's own top comment for the Windows history.
/// `(available, reason_if_not)`, matching `installer::check_installer_availability`'s own
/// shape. Always available now (no more platform gating), kept as a function rather than
/// inlined `true` at each call site since a real, newly-discovered platform gap should have
/// one place to report from, the same as `check_installer_availability` does for installers.
pub fn check_android_availability() -> (bool, Option<String>) {
    (true, None)
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

pub(crate) async fn download_with_retries(
    url: &str,
    dest: &Path,
    on_progress: &mut (impl FnMut(f64) + Send),
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

pub(crate) fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
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

/// The JDK download extracts to a single, version-named top-level folder (e.g.
/// `jdk-21.0.12.1+1-jre/`) whose exact name isn't worth hardcoding -- this finds it instead
/// of guessing.
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
    } else if cfg!(target_os = "windows") {
        "windows"
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

/// Downloads (once; cached) a portable Eclipse Temurin **JDK** and returns `JAVA_HOME` --
/// Gradle needs a JVM to run at all, and Packmaster's whole point is that a game developer
/// doesn't need one preinstalled (see `shell.rs`'s own doc comment on the same principle for
/// the engine shell itself).
///
/// A plain JRE (this function's own original choice, and still what its name/cache-folder/
/// progress-phase-key say -- kept as-is to avoid an unrelated i18n churn across all 9 locale
/// files for a wording nitpick, see the CLAUDE.md rule on that) is **not enough**: confirmed
/// via a real Windows build failure, Gradle 9.5.1's own version-catalog codegen
/// (`GeneratedClassCompilationException`) needs `javac`, which a JRE doesn't ship at all --
/// `.github/workflows/android.yml` avoids this because `actions/setup-java` defaults to
/// installing a real JDK, not a JRE, a distinction that was easy to miss porting the same
/// bootstrap to a from-scratch Adoptium download here. The cache folder name was changed from
/// `jre` to `jdk` specifically so this fix self-heals for anyone who already has the old,
/// JDK-less download cached from before this fix -- reusing the same folder name would have
/// kept serving the broken cached JRE forever.
pub async fn ensure_jre(app: &AppHandle, mut on_progress: impl FnMut(f64) + Send) -> Result<PathBuf, String> {
    let (os, arch) = adoptium_os_arch()?;
    let cache_dir = tools_cache_dir(app)?.join("jdk").join(format!("{os}-{arch}"));
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
        "https://api.adoptium.net/v3/binary/latest/{JRE_FEATURE_VERSION}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse"
    );
    // Adoptium packages Windows binaries as .zip, everything else as .tar.gz.
    if os == "windows" {
        let archive_path = cache_dir.join("jdk.zip");
        download_with_retries(&url, &archive_path, &mut on_progress).await.map_err(|e| format!("downloading JDK: {e}"))?;
        extract_zip(&archive_path, &cache_dir)?;
        tokio::fs::remove_file(&archive_path).await.ok();
    } else {
        let archive_path = cache_dir.join("jdk.tar.gz");
        download_with_retries(&url, &archive_path, &mut on_progress).await.map_err(|e| format!("downloading JDK: {e}"))?;
        extract_tar_gz(&archive_path, &cache_dir)?;
        tokio::fs::remove_file(&archive_path).await.ok();
    }
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
    } else if cfg!(target_os = "windows") {
        Ok("win")
    } else {
        Err("unsupported OS for the Android SDK".to_string())
    }
}

/// `sdkmanager` (and every other cmdline-tools script) ships as a `.bat` on Windows, a
/// plain extension-less shell script everywhere else -- same distinction as the engine's own
/// `gradlew`/`gradlew.bat` (see CUSTOMIZATIONS.md).
fn sdkmanager_path(cmdline_tools_dir: &Path) -> PathBuf {
    cmdline_tools_dir.join("bin").join(if cfg!(windows) { "sdkmanager.bat" } else { "sdkmanager" })
}

/// `.bat` files aren't directly executable via `std::process::Command` on Windows the way a
/// real `.exe` is -- `CreateProcess` doesn't resolve script interpreters on its own the way
/// `cmd.exe` typing a bare name does. Routing through `cmd /C` explicitly is the standard,
/// robust fix (same approach Cargo/rustup's own Windows batch-script wrappers use). A no-op
/// everywhere else: returns a plain `Command::new(program)`, unchanged.
pub(crate) fn script_command(program: &Path) -> std::process::Command {
    if cfg!(windows) && program.extension().and_then(|e| e.to_str()) == Some("bat") {
        let mut command = std::process::Command::new("cmd");
        command.arg("/C").arg(program);
        command
    } else {
        std::process::Command::new(program)
    }
}

/// Runs `command` to completion, capturing stdout/stderr instead of letting them inherit the
/// parent's -- necessary because Packmaster is a windowed (console-less) GUI app on every
/// platform, same root cause as the engine repo's own `windows_subsystem` gap: a plain
/// `.status()` call's inherited stdio goes nowhere visible, so `sdkmanager`/`gradlew` failing
/// for any real reason (a rejected license, a network hiccup, a Java version mismatch, ...)
/// previously surfaced to the user as nothing but a bare exit code, with the actual "why"
/// silently discarded. On failure, folds a tail of whatever the process printed into the
/// returned error so it's at least visible in the generic error banner, even without a log
/// file to go dig through.
pub(crate) fn run_capturing_output(command: &mut std::process::Command, context: &str) -> Result<(), String> {
    use std::process::Stdio;

    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("running {context}: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format_process_failure(context, &output.status, &output.stdout, &output.stderr))
}

/// Shared by `run_capturing_output` and `accept_sdk_licenses` (which needs to write to stdin
/// while the process runs, so it can't use `Command::output()` directly and builds its own
/// `std::process::Output` via `wait_with_output` instead).
fn format_process_failure(context: &str, status: &std::process::ExitStatus, stdout: &[u8], stderr: &[u8]) -> String {
    let mut combined = String::from_utf8_lossy(stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(stderr));
    let trimmed = combined.trim();
    // Keep only the tail -- sdkmanager/gradlew can print a lot of unrelated noise (download
    // progress, deprecation warnings) before the actual error, and this whole string renders
    // as a single error banner in the UI.
    const MAX_LEN: usize = 4000;
    let tail = if trimmed.len() > MAX_LEN { &trimmed[trimmed.len() - MAX_LEN..] } else { trimmed };
    if tail.is_empty() {
        format!("{context} exited with {status}")
    } else {
        format!("{context} exited with {status} -- output:\n{tail}")
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

    let sdkmanager = sdkmanager_path(&cmdline_tools_dir);
    accept_sdk_licenses(&sdkmanager, &sdk_root, java_home)?;
    let platform_package = resolve_platform_package(&sdkmanager, &sdk_root, java_home)?;
    run_sdkmanager(
        &sdkmanager,
        &sdk_root,
        java_home,
        &["platform-tools", &platform_package, &format!("build-tools;{BUILD_TOOLS_VERSION}")],
    )?;

    tokio::fs::write(&marker, b"1").await.map_err(|e| e.to_string())?;
    Ok(sdk_root)
}

/// As of Google's current repository (confirmed directly, not from documentation, 2026-09-11),
/// `platforms;android-{ANDROID_PLATFORM}` alone is **not a real package** for platform levels
/// 37+: Google now versions these as `platforms;android-37.0`/`.1`/`.2` (plus `-betaN`
/// prereleases and separate `-extN` SDK-extension packages), the same way older levels like 35
/// got `-ext14`/`-ext15` add-ons alongside the bare package -- except for 37+ the *bare* form
/// doesn't exist at all. Hardcoding the literal string (this function's previous approach) is
/// exactly what produced a real, confirmed "Failed to find package 'platforms;android-37'" on
/// a real Windows machine even after fixing the separate `--channel=3` gap (needed for these
/// to be visible in the first place, but not sufficient on its own).
///
/// Mirrors `.github/workflows/android.yml`'s own dynamic resolution (`sdkmanager --list |
/// grep -oE "platforms;android-${ANDROID_COMPILE_SDK}(\.[0-9]+)?(-ext[0-9]+)?" | grep -v --
/// '-beta\|-rc' | sort -V | tail -1`) instead of trying to guess/hardcode the right exact
/// string here too -- list what's actually offered, keep only non-prerelease matches for this
/// platform level, and take the highest `.N` (falling back to the highest `-extN`, then the
/// bare form, if that's all that's offered instead).
fn resolve_platform_package(sdkmanager: &Path, sdk_root: &Path, java_home: &Path) -> Result<String, String> {
    use std::process::Stdio;

    let mut command = script_command(sdkmanager);
    command
        .arg(format!("--sdk_root={}", sdk_root.display()))
        .arg("--channel=3")
        .arg("--list")
        .env("JAVA_HOME", java_home);
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("running sdkmanager --list: {e}"))?;
    if !output.status.success() {
        return Err(format_process_failure("sdkmanager --list", &output.status, &output.stdout, &output.stderr));
    }

    let prefix = format!("platforms;android-{ANDROID_PLATFORM}");
    let mut bare: Option<String> = None;
    let mut best_dot: Option<(u32, String)> = None;
    let mut best_ext: Option<(u32, String)> = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some(token) = line.trim_start().split(|c: char| c.is_whitespace() || c == '|').next() else {
            continue;
        };
        if token == prefix {
            bare = Some(token.to_string());
            continue;
        }
        let Some(suffix) = token.strip_prefix(&prefix) else {
            continue;
        };
        if suffix.contains("beta") || suffix.contains("rc") {
            continue; // pre-release channel -- android.yml's own `grep -v` excludes these too
        }
        if let Some(n) = suffix.strip_prefix('.').and_then(|n| n.parse::<u32>().ok()) {
            if best_dot.as_ref().map_or(true, |(m, _)| n > *m) {
                best_dot = Some((n, token.to_string()));
            }
        } else if let Some(n) = suffix.strip_prefix("-ext").and_then(|n| n.parse::<u32>().ok()) {
            if best_ext.as_ref().map_or(true, |(m, _)| n > *m) {
                best_ext = Some((n, token.to_string()));
            }
        }
    }

    best_dot
        .or(best_ext)
        .map(|(_, pkg)| pkg)
        .or(bare)
        .ok_or_else(|| format!("no {prefix}* package advertised by sdkmanager --list"))
}

/// `sdkmanager --licenses` prompts once per not-yet-accepted license, reading `y`/`d` from
/// stdin -- piping a generous number of `y\n` lines has the same effect as CI's own
/// `yes | sdkmanager --licenses` without depending on the separate `yes` binary existing.
fn accept_sdk_licenses(sdkmanager: &Path, sdk_root: &Path, java_home: &Path) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = script_command(sdkmanager)
        .arg(format!("--sdk_root={}", sdk_root.display()))
        // See `run_sdkmanager`'s own comment on `--channel=3` -- without it, sdkmanager only
        // looks at the stable channel, and never even offers the license for whatever's not
        // in it (silently, not an error) for this step to accept ahead of time.
        .arg("--channel=3")
        .arg("--licenses")
        .env("JAVA_HOME", java_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("running sdkmanager --licenses: {e}"))?;
    // Writing on a separate thread rather than inline before `wait_with_output` below: stdout
    // and stderr are now real pipes (needed to capture them for `format_process_failure`), not
    // `Stdio::null()` -- writing "y\n" x32 synchronously here, before anything starts draining
    // those pipes, risks a classic deadlock if sdkmanager's license-prompt output is ever large
    // enough to fill the OS pipe buffer while it's still blocked waiting for us to read more
    // stdin than we've written. Concurrent write + drain avoids that regardless of how much
    // either side produces.
    let mut stdin = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(stdin) = stdin.as_mut() {
            // Comfortably more than the number of distinct SDK licenses that have ever
            // existed. Dropping `stdin` (out of scope at the end of this closure) closes the
            // pipe, or sdkmanager blocks forever waiting for EOF after its last prompt instead
            // of exiting -- same reason `yes | sdkmanager --licenses` relies on `yes`
            // eventually getting SIGPIPE'd rather than sdkmanager reading from an
            // indefinitely-open pipe.
            let _ = stdin.write_all("y\n".repeat(32).as_bytes());
        }
    });
    // `wait_with_output` (not `wait`) so a rejected/unexpected license prompt -- previously
    // discarded via `Stdio::null()` -- is captured and surfaces in the error instead of just
    // a bare exit code. See `run_capturing_output`'s own doc comment for why this matters on
    // a console-less GUI app.
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = writer.join();
    if !output.status.success() {
        return Err(format_process_failure("sdkmanager --licenses", &output.status, &output.stdout, &output.stderr));
    }
    Ok(())
}

fn run_sdkmanager(sdkmanager: &Path, sdk_root: &Path, java_home: &Path, packages: &[&str]) -> Result<(), String> {
    let mut command = script_command(sdkmanager);
    command
        .arg(format!("--sdk_root={}", sdk_root.display()))
        // The engine's own `.github/workflows/android.yml` needs this same flag for the exact
        // same package set, with its own comment explaining why: sdkmanager only considers the
        // stable channel by default, and `platforms;android-{ANDROID_PLATFORM}` (a preview/
        // canary-only SDK level as of this writing) isn't in it -- omitting this flag is
        // exactly what produced a real "Failed to find package" failure on a real Windows
        // machine, since without it the package is invisible to sdkmanager, not just
        // unlicensed. `--channel=3` is canary, the widest channel (superset of stable/beta/
        // dev), matching that workflow's own choice so this doesn't drift out of sync with it.
        .arg("--channel=3")
        .env("JAVA_HOME", java_home);
    for package in packages {
        command.arg(package);
    }
    run_capturing_output(&mut command, &format!("sdkmanager while installing {packages:?}"))
}

// ── Android project (from the engine's rolling "test" release) ─────────────────────────

fn test_release_asset_url(asset_name: &str) -> String {
    format!("https://github.com/{ANDROID_REPO}/releases/download/{ANDROID_TEST_RELEASE_TAG}/{asset_name}")
}

/// Downloads (once per app run -- see the "always re-check" note below; not cached across
/// runs the way the JRE/SDK are) the engine's `support/android/apk/` Gradle project. Unlike
/// the JRE/SDK toolchains (pinned/self-updating, safe to cache indefinitely), this tracks a
/// rolling release that's overwritten on every push to the engine's `main` branch -- caching
/// it would silently keep building against a stale snapshot of the engine's own Android
/// integration.
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
    // support/android/apk`) -- the exact relative path this function's own caller expects
    // (`apk_project_dir` below), with `dest_dir` playing the role of the engine repo root.
    Ok(dest_dir)
}

// ── Orchestration ────────────────────────────────────────────────────────────────────────

pub struct AndroidBuildOptions<'a> {
    pub content_dir: &'a Path,
    pub icon_png: Option<&'a Path>,
    pub app_name_override: &'a str,
    pub orientation_override: &'a str,
    /// `Some` builds a real, signed release `.apk` (Gradle's `assemble<Arch>Release`,
    /// signed via the 4 env vars `SigningConfig::env_vars` sets) instead of the default
    /// debug one -- mirrors the engine's own `mach bundle --android-release`, see
    /// `signing.rs`'s own doc comment for the full mechanism.
    pub signing: Option<&'a crate::signing::SigningConfig>,
}

/// Builds a debug (or, with `signing`, a signed release) `.apk` and returns its path.
/// `on_progress(phase, fraction)` mirrors `bundle.rs`'s own per-platform progress events,
/// with `"android"` as the pseudo-platform name -- see that module's `emit_progress`. Mirrors
/// the engine's own `_bundle_android` (`python/servo/post_build_commands.py`) step for step --
/// see that function's own doc comment for the design this ports to Rust; no native library
/// or NDK involved anymore, just copying content into a Gradle project and running it.
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

    on_progress("downloading-project", 0.0);
    let project_root = download_android_project(app, |f| on_progress("downloading-project", f)).await?;

    on_progress("assembling", 0.0);
    let scratch_root = std::env::temp_dir().join(format!("roves-packmaster-android-{}", std::process::id()));
    if scratch_root.exists() {
        std::fs::remove_dir_all(&scratch_root).map_err(|e| e.to_string())?;
    }
    // `project_root` already has the `support/android/apk/...` layout (see
    // `download_android_project`'s own comment) -- moving it wholesale into place is enough,
    // no need to know or reconstruct the individual module paths inside it. A scratch copy
    // rather than building the download cache in place, same reasoning as the engine's own
    // `_bundle_android`: a repeat build for a different game starts from a clean copy every
    // time instead of accumulating the previous game's files.
    copy_dir_recursive(&project_root, &scratch_root)?;
    let apk_project_dir = scratch_root.join("support").join("android").join("apk");
    if !apk_project_dir.is_dir() {
        return Err(format!("downloaded Android project has no support/android/apk/ under {scratch_root:?}"));
    }

    // Content: mirrors `_bundle_android`'s unconditional `shutil.copytree(content_dir,
    // assets_dir)` -- no compression option exists for Android today (MainActivity.kt serves
    // the game via `WebViewAssetLoader` from this exact `assets/www/` folder, not a packed
    // archive).
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

    let gradle_build_type = if options.signing.is_some() { "Release" } else { "Debug" };
    let task = format!(":servoapp:assemble{ARCH_STRING}{gradle_build_type}");
    let mut command = script_command(&gradlew);
    command
        .current_dir(&apk_project_dir)
        .arg("--no-daemon")
        .arg(&task)
        .arg(format!("-PservoScreenOrientation={orientation}"))
        .arg(format!("-PservoAppName={app_name}"))
        .env("JAVA_HOME", &java_home)
        .env("ANDROID_SDK_ROOT", &sdk_root)
        .env("ANDROID_HOME", &sdk_root);
    // See `signing.rs`'s own doc comment: these 4 vars are the entire signing mechanism,
    // read directly by upstream Servo's own (unpatched) buildSrc/Android.kt.
    if let Some(signing) = options.signing {
        for (key, value) in signing.env_vars() {
            command.env(key, value);
        }
    }
    run_capturing_output(&mut command, &format!("gradlew while running {task}"))?;

    // AGP's own standard per-variant output location -- no more custom
    // `copyAndRename<Variant>APK` Gradle task moving it elsewhere (see the engine's own
    // `_bundle_android`, which made the identical switch for the identical reason: nothing
    // upstream needed that task once there was no `target/<rust-triple>/` for it to reach
    // back into). Exactly one match expected -- more than one is ambiguous, zero means
    // gradlew's own "successful" exit code didn't actually produce anything.
    let pattern = apk_project_dir.join("servoapp").join("build").join("outputs").join("apk").join("**").join("*.apk");
    let matches: Vec<PathBuf> = glob::glob(&pattern.to_string_lossy())
        .map_err(|e| format!("invalid apk glob pattern: {e}"))?
        .filter_map(|entry| entry.ok())
        .collect();
    let [apk_path] = matches.as_slice() else {
        return Err(format!("expected exactly one built .apk under {pattern:?}, found {}", matches.len()));
    };
    on_progress("done", 1.0);
    Ok(apk_path.clone())
}

pub(crate) fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
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
