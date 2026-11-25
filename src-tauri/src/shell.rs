//! Fetches and caches the prebuilt Roves desktop player packages instead of compiling one --
//! the whole point of Packmaster is that a game developer using it needs no Rust/Python
//! toolchain (see this project's own README/CLAUDE.md). See the engine repo's own AGENTS.md,
//! "Cutting a versioned release" section: src/lib/shell-version.txt is the shared pin, read
//! here with include_str! and by the frontend with Vite's raw loader.
//!
//! Since engine v0.9.0 the desktop release assets are the CEF players (`cef_player.rs` for
//! their layout): `roves_shell_<platform>[_steam|_mcp].zip`. The cache keeps the downloaded
//! zip itself, never an extraction: `cef_player::assemble` copies its entries into the output
//! zip as they are, so the Unix modes (executables, the setuid `chrome-sandbox`) and any
//! symlink survive whatever host Packmaster runs on.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

use crate::cef_player::Platform;

fn pinned_shell_version() -> &'static str {
    include_str!("../../src/lib/shell-version.txt").trim()
}

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/DRincs-Productions/roves/releases/latest";

/// True only when *this exact build of Packmaster* was itself produced by `roves-packmaster`'s own
/// `test.yml` (which sets `PACKMASTER_TEST_BUILD=1` before `npm run tauri build` — see that
/// workflow), never by inspecting anything at runtime: a shipped, tagged Packmaster release
/// must behave identically regardless of whatever's in an end user's own environment.
fn is_test_build() -> bool {
    option_env!("PACKMASTER_TEST_BUILD").is_some()
}

/// A real, tagged Packmaster release always targets the shared pinned shell version,
/// so the same build reliably produces the same output and can safely cache what it
/// downloads. A *test* build instead targets whichever tag GitHub currently reports as the
/// engine repo's latest release — test builds are rebuilt on every push anyway, so there's
/// no reproducibility to protect, and always following "latest" means testing Packmaster
/// against a newly cut engine release doesn't need a shared-pin bump just to
/// notice it. Falls back to the pinned version if the lookup itself fails (offline,
/// GitHub API rate-limited, etc.) — same "best-effort, never block on this" reasoning as the
/// frontend's own `shell-version.ts`, `checkForNewShellVersion`.
pub async fn resolve_shell_version() -> String {
    if !is_test_build() {
        return pinned_shell_version().to_string();
    }
    fetch_latest_release_tag().await.unwrap_or_else(|| pinned_shell_version().to_string())
}

async fn fetch_latest_release_tag() -> Option<String> {
    // GitHub's API rejects an unauthenticated request with no User-Agent header at all.
    let text = reqwest::Client::new()
        .get(LATEST_RELEASE_API_URL)
        .header("User-Agent", "roves-packmaster")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("tag_name")?.as_str().map(str::to_string)
}

/// The engine release asset for a platform and variant: `roves_shell_<platform>.zip`, the
/// Steam build `..._steam.zip`, or the Windows-only local-MCP development build
/// `roves_shell_windows_mcp.zip`. The engine publishes no Steam + MCP package and no MCP
/// package for macOS/Linux, so those combinations are errors, never a silent fallback.
pub fn asset_name(platform: Platform, steam: bool, mcp: bool) -> Result<String, String> {
    let suffix = match (steam, mcp) {
        (false, false) => "",
        (true, false) => "_steam",
        (false, true) if platform == Platform::Windows => "_mcp",
        (false, true) => {
            return Err(format!(
                "the local MCP bridge only exists in the Windows player, not for {}",
                platform.id()
            ));
        },
        (true, true) => {
            return Err(
                "the local MCP bridge and Steam can't be combined: the MCP-enabled Windows player has no Steam support"
                    .to_string(),
            );
        },
    };
    Ok(format!("roves_shell_{}{suffix}.zip", platform.id()))
}

pub fn asset_url(asset: &str, version: &str) -> String {
    format!("https://github.com/DRincs-Productions/roves/releases/download/{version}/{asset}")
}

/// Real availability check (not just an assumption) that the release this build of
/// Packmaster targets actually has `asset` published — a HEAD request against the real
/// download URL, so a broken/retracted release surfaces as "can't distribute this" instead
/// of a confusing failure partway through generation.
pub async fn is_asset_available(asset: &str) -> bool {
    let version = resolve_shell_version().await;
    reqwest::Client::new()
        .head(asset_url(asset, &version))
        .send()
        .await
        .map(|response| response.status().is_success() || response.status().is_redirection())
        .unwrap_or(false)
}

/// Downloads (or reuses an already-cached copy of) `asset` and returns the path of the zip.
/// A downloaded zip is accepted only once its central directory reads back, so a truncated
/// download is retried instead of being cached.
pub async fn ensure_asset(
    app: &AppHandle,
    asset: &str,
    mut on_progress: impl FnMut(f64) + Send,
) -> Result<PathBuf, String> {
    let version = resolve_shell_version().await;
    let cache_dir = shells_cache_dir(app)?
        .join(&version)
        .join(asset.trim_end_matches(".zip"));
    let zip_path = cache_dir.join(asset);
    let marker = cache_dir.join(".complete");
    // A test build always targets whichever tag is currently "latest" (see
    // resolve_shell_version) -- reusing a cached download here would silently keep testing
    // against whatever was latest the first time this ran, even after a newer engine release
    // ships. A real, tagged Packmaster release targets an immutable, pinned tag instead,
    // where caching has no such staleness risk.
    if !is_test_build() && marker.exists() && zip_path.exists() {
        on_progress(1.0);
        return Ok(zip_path);
    }

    if cache_dir.exists() {
        tokio::fs::remove_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;

    let url = asset_url(asset, &version);
    // The player zip is 100+ MB -- a mid-stream connection drop (flaky wifi, a corporate
    // TLS-inspecting proxy resetting a long-lived download) surfaces from reqwest as a
    // generic "error decoding response body" with no further detail, and is exactly the
    // kind of transient failure a retry fixes; a genuinely bad URL/404 fails immediately on
    // the first attempt instead (see download_file's status check), so this doesn't mask
    // real errors, just network flakiness.
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        // A connection that closes early without reqwest noticing produces a truncated-but-
        // not-erroring zip, which only surfaces once its central directory is read.
        let result = match download_file(&url, &zip_path, &mut on_progress).await {
            Ok(()) => check_zip(&zip_path),
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
        return Err(format!("downloading {asset} after {MAX_ATTEMPTS} attempts: {last_err}"));
    }
    tokio::fs::write(&marker, b"1").await.map_err(|e| e.to_string())?;
    Ok(zip_path)
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
    file.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

fn check_zip(zip_path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(file)
        .map(|_| ())
        .map_err(|e| format!("reading the downloaded zip: {e}"))
}

fn shells_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("shells"))
}

/// Total on-disk size of every cached player package, across every version/asset ever
/// downloaded (`ensure_asset` keys each one by version -- so this walks all of them, not just
/// whatever `resolve_shell_version` currently targets).
pub fn cache_size(app: &AppHandle) -> Result<u64, String> {
    let dir = shells_cache_dir(app)?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(&dir) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_file() {
            total += entry.metadata().map_err(|e| e.to_string())?.len();
        }
    }
    Ok(total)
}

/// Removes every cached player package outright, regardless of version -- the next
/// `ensure_asset` call just re-downloads fresh. Not scoped to "only the stale/unused ones":
/// simplest correct behavior, and a stale version already never gets served (see
/// `ensure_asset`'s version-keyed cache_dir) -- this only reclaims disk space.
pub fn clear_cache(app: &AppHandle) -> Result<(), String> {
    let dir = shells_cache_dir(app)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_cover_every_platform_and_variant() {
        use Platform::*;
        assert_eq!(asset_name(Windows, false, false).unwrap(), "roves_shell_windows.zip");
        assert_eq!(asset_name(Windows, true, false).unwrap(), "roves_shell_windows_steam.zip");
        assert_eq!(asset_name(Windows, false, true).unwrap(), "roves_shell_windows_mcp.zip");
        assert!(asset_name(Windows, true, true).unwrap_err().contains("Steam"));
        for (platform, id) in [(Macos, "macos"), (Linux, "linux")] {
            assert_eq!(asset_name(platform, false, false).unwrap(), format!("roves_shell_{id}.zip"));
            assert_eq!(asset_name(platform, true, false).unwrap(), format!("roves_shell_{id}_steam.zip"));
            assert!(asset_name(platform, false, true)
                .unwrap_err()
                .contains("only exists in the Windows player"));
            assert!(asset_name(platform, true, true).is_err());
        }
    }

    #[test]
    fn asset_url_uses_the_release_download_scheme() {
        assert_eq!(
            asset_url("roves_shell_linux_steam.zip", "v0.9.0"),
            "https://github.com/DRincs-Productions/roves/releases/download/v0.9.0/roves_shell_linux_steam.zip"
        );
    }
}
