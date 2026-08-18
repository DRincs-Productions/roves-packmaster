//! Fetches and caches the prebuilt Roves engine shell instead of compiling one — the whole
//! point of Packmaster is that a game developer using it needs no Rust/Python toolchain (see
//! this project's own README/CLAUDE.md). See the engine repo's own CLAUDE.md, "Cutting a
//! versioned release" section: `TARGET_SHELL_VERSION` here must be bumped in step with
//! `roves-ui`'s frontend `src/lib/shell-version.ts` constant of the same name whenever a new
//! shell version is published.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

pub const TARGET_SHELL_VERSION: &str = "v0.2.0";

/// The rolling, unversioned tag the *engine's own* `test.yml` publishes a fresh shell build
/// to on every push to `main` (see that workflow's `TEST_RELEASE_TAG`) — distinct from a real
/// tagged release, its asset content changes over time under the same tag name.
const TEST_SHELL_TAG: &str = "test";

/// True only when *this exact build of Packmaster* was itself produced by `roves-ui`'s own
/// `test.yml` (which sets `PACKMASTER_TEST_BUILD=1` before `npm run tauri build` — see that
/// workflow), never by inspecting anything at runtime: a shipped, tagged Packmaster release
/// must behave identically regardless of whatever's in an end user's own environment.
fn is_test_build() -> bool {
    option_env!("PACKMASTER_TEST_BUILD").is_some()
}

/// The engine release tag this build of Packmaster targets: the exact, pinned
/// `TARGET_SHELL_VERSION` for a real Packmaster release, so the same build always produces
/// the same output and can safely cache what it downloads -- but the engine's own rolling
/// `test` tag for a Packmaster *test* build, so testing Packmaster against engine changes
/// doesn't require re-tagging an engine release for every iteration.
pub fn target_shell_version() -> &'static str {
    if is_test_build() { TEST_SHELL_TAG } else { TARGET_SHELL_VERSION }
}

/// `windows` | `macos` | `linux` — matches release.yml's own `matrix.os_name` and the
/// `roves_shell_<os_name>.zip`/`roves_shell_<os_name>_steam.zip` asset naming (the engine's
/// own `matrix.asset_suffix`).
pub fn shell_asset_url(platform: &str, steam: bool) -> String {
    let suffix = if steam { "_steam" } else { "" };
    let version = target_shell_version();
    format!("https://github.com/DRincs-Productions/roves/releases/download/{version}/roves_shell_{platform}{suffix}.zip")
}

/// Real availability check (not just an assumption) that the shell release this build of
/// Packmaster targets actually has a published asset for `platform` (and Steam variant, if
/// requested) — a HEAD request against the real download URL, so a broken/retracted release
/// surfaces as "can't distribute this" instead of a confusing failure partway through
/// generation.
pub async fn is_shell_available(platform: &str, steam: bool) -> bool {
    reqwest::Client::new()
        .head(shell_asset_url(platform, steam))
        .send()
        .await
        .map(|response| response.status().is_success() || response.status().is_redirection())
        .unwrap_or(false)
}

/// Downloads (or reuses an already-cached copy of) the shell for `platform`, extracting it
/// into this app's cache dir. Returns the path to the extracted `roves/` folder — see
/// `.github/workflows/release.yml`'s own zip step in the engine repo: every zip's single
/// top-level folder is named after `PACKAGE_NAME` ("roves"), containing exactly what `mach
/// bundle --output` produced (flat `play.exe`/`play` + deps on Windows/Linux, `play.app/` on
/// macOS).
pub async fn ensure_shell(
    app: &AppHandle,
    platform: &str,
    steam: bool,
    mut on_progress: impl FnMut(f64) + Send,
) -> Result<PathBuf, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("shells")
        .join(target_shell_version())
        .join(platform)
        // Plain and Steam-enabled shells are different binaries entirely -- keying the cache
        // by variant too means toggling Steam on/off never reuses (or clobbers) the wrong
        // cached extraction.
        .join(if steam { "steam" } else { "plain" });
    let extracted_root = cache_dir.join("roves");
    let marker = cache_dir.join(".complete");
    // A test build's shell lives under the engine's rolling `test` tag, whose actual asset
    // content changes over time under that same tag name -- reusing a cached extraction here
    // would silently test against a stale engine build. A real, tagged Packmaster release
    // targets an immutable, pinned tag instead, where caching has no such staleness risk.
    if !is_test_build() && marker.exists() && extracted_root.exists() {
        on_progress(1.0);
        return Ok(extracted_root);
    }

    if cache_dir.exists() {
        tokio::fs::remove_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;

    let url = shell_asset_url(platform, steam);
    let zip_path = cache_dir.join("shell.zip");
    // The shell zip is 100+ MB -- a mid-stream connection drop (flaky wifi, a corporate
    // TLS-inspecting proxy resetting a long-lived download) surfaces from reqwest as a
    // generic "error decoding response body" with no further detail, and is exactly the
    // kind of transient failure a retry fixes; a genuinely bad URL/404 fails immediately on
    // the first attempt instead (see download_file's status check), so this doesn't mask
    // real errors, just network flakiness.
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        // Extraction is retried alongside the download, not just the download itself -- a
        // connection that closes early without reqwest noticing produces a truncated-but-not-
        // erroring zip, which only surfaces once `extract_zip` tries to read it.
        let result = match download_file(&url, &zip_path, &mut on_progress).await {
            Ok(()) => extract_zip(&zip_path, &cache_dir),
            Err(e) => Err(e),
        };
        match result {
            Ok(()) => {
                last_err.clear();
                break;
            },
            Err(e) => {
                last_err = e;
                tokio::fs::remove_file(&zip_path).await.ok();
                if attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                }
            },
        }
    }
    if !last_err.is_empty() {
        return Err(format!("downloading shell after {MAX_ATTEMPTS} attempts: {last_err}"));
    }
    tokio::fs::remove_file(&zip_path).await.ok();
    tokio::fs::write(&marker, b"1").await.map_err(|e| e.to_string())?;

    if !extracted_root.exists() {
        return Err(format!(
            "downloaded shell zip for {platform} didn't contain the expected roves/ folder"
        ));
    }
    Ok(extracted_root)
}

async fn download_file(url: &str, dest: &Path, on_progress: &mut impl FnMut(f64)) -> Result<(), String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("downloading {url}: {e}"))?;
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

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive
        .extract(dest_dir)
        .map_err(|e| format!("extracting shell zip: {e}"))
}
