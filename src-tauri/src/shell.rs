//! Fetches and caches the prebuilt Roves engine shell instead of compiling one — the whole
//! point of Packmaster is that a game developer using it needs no Rust/Python toolchain (see
//! this project's own README/CLAUDE.md). See the engine repo's own CLAUDE.md, "Cutting a
//! versioned release" section: `TARGET_SHELL_VERSION` here must be bumped in step with
//! `roves-packmaster`'s frontend `src/lib/shell-version.ts` constant of the same name whenever a new
//! shell version is published.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

pub const TARGET_SHELL_VERSION: &str = "v0.4.9";

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/DRincs-Productions/roves/releases/latest";

/// True only when *this exact build of Packmaster* was itself produced by `roves-packmaster`'s own
/// `test.yml` (which sets `PACKMASTER_TEST_BUILD=1` before `npm run tauri build` — see that
/// workflow), never by inspecting anything at runtime: a shipped, tagged Packmaster release
/// must behave identically regardless of whatever's in an end user's own environment.
fn is_test_build() -> bool {
    option_env!("PACKMASTER_TEST_BUILD").is_some()
}

/// A real, tagged Packmaster release always targets the exact pinned `TARGET_SHELL_VERSION`,
/// so the same build reliably produces the same output and can safely cache what it
/// downloads. A *test* build instead targets whichever tag GitHub currently reports as the
/// engine repo's latest release — test builds are rebuilt on every push anyway, so there's
/// no reproducibility to protect, and always following "latest" means testing Packmaster
/// against a newly cut engine release doesn't need a `TARGET_SHELL_VERSION` bump here just to
/// notice it. Falls back to `TARGET_SHELL_VERSION` if the lookup itself fails (offline,
/// GitHub API rate-limited, etc.) — same "best-effort, never block on this" reasoning as the
/// frontend's own `shell-version.ts`, `checkForNewShellVersion`.
///
/// (An earlier version of this pointed a test build at the engine's own rolling `test` tag
/// instead, assuming it published a bare `roves_shell_<platform>.zip` the way a real release
/// does — it doesn't, so that found nothing published for any platform. "Latest real
/// release" is what actually exists to target.)
pub async fn resolve_shell_version() -> String {
    if !is_test_build() {
        return TARGET_SHELL_VERSION.to_string();
    }
    fetch_latest_release_tag().await.unwrap_or_else(|| TARGET_SHELL_VERSION.to_string())
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

/// `windows` | `macos` | `linux` — matches release.yml's own `matrix.os_name` and the
/// `roves_shell_<os_name>.zip`/`roves_shell_<os_name>_steam.zip` asset naming (the engine's
/// own `matrix.asset_suffix`).
pub fn shell_asset_url(platform: &str, steam: bool, version: &str) -> String {
    let suffix = if steam { "_steam" } else { "" };
    format!("https://github.com/DRincs-Productions/roves/releases/download/{version}/roves_shell_{platform}{suffix}.zip")
}

/// Real availability check (not just an assumption) that the shell release this build of
/// Packmaster targets actually has a published asset for `platform` (and Steam variant, if
/// requested) — a HEAD request against the real download URL, so a broken/retracted release
/// surfaces as "can't distribute this" instead of a confusing failure partway through
/// generation.
pub async fn is_shell_available(platform: &str, steam: bool) -> bool {
    let version = resolve_shell_version().await;
    reqwest::Client::new()
        .head(shell_asset_url(platform, steam, &version))
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
    let version = resolve_shell_version().await;
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("shells")
        .join(&version)
        .join(platform)
        // Plain and Steam-enabled shells are different binaries entirely -- keying the cache
        // by variant too means toggling Steam on/off never reuses (or clobbers) the wrong
        // cached extraction.
        .join(if steam { "steam" } else { "plain" });
    let extracted_root = cache_dir.join("roves");
    let marker = cache_dir.join(".complete");
    // A test build always targets whichever tag is currently "latest" (see
    // resolve_shell_version) -- reusing a cached extraction here would silently keep testing
    // against whatever was latest the first time this ran, even after a newer engine release
    // ships. A real, tagged Packmaster release targets an immutable, pinned tag instead,
    // where caching has no such staleness risk.
    if !is_test_build() && marker.exists() && extracted_root.exists() {
        on_progress(1.0);
        return Ok(extracted_root);
    }

    if cache_dir.exists() {
        tokio::fs::remove_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;

    let url = shell_asset_url(platform, steam, &version);
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

fn shells_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("shells"))
}

/// Total on-disk size of every cached shell extraction, across every version/platform/variant
/// ever downloaded (`ensure_shell` keys each one by version — see that function's own cache_dir
/// — so this walks all of them, not just whatever `resolve_shell_version` currently targets).
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

/// Removes every cached shell extraction outright, regardless of version -- the next
/// `ensure_shell` call for any version just re-downloads fresh. Not scoped to "only the
/// stale/unused ones": simplest correct behavior, and a stale version already never gets
/// served (see `ensure_shell`'s version-keyed cache_dir) -- this only reclaims disk space,
/// it isn't needed for correctness.
pub fn clear_cache(app: &AppHandle) -> Result<(), String> {
    let dir = shells_cache_dir(app)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}
