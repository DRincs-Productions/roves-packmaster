//! The engine release the desktop packages are built from, and the cache of its player
//! packages. The packaging itself (download, verification, assembly) is done by the roves-cli
//! sidecar (`roves_cli.rs`); this module decides which release it targets, checks that the
//! release really publishes an asset (for the live availability badges), and owns the cache
//! folder the sidecar downloads into (`ROVES_CACHE_DIR`), so the cache size/clear controls keep
//! working. src/lib/shell-version.txt is the shared pin, read here with include_str! and by the
//! frontend with Vite's raw loader (see the engine repo's AGENTS.md, "Cutting a versioned
//! release").

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// A desktop target of the engine's player packages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "windows" => Some(Self::Windows),
            "macos" => Some(Self::Macos),
            "linux" => Some(Self::Linux),
            _ => None,
        }
    }

    /// `windows` | `macos` | `linux`, as in the engine's release asset names and roves-cli's
    /// `--platform`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
        }
    }
}

fn pinned_shell_version() -> &'static str {
    include_str!("../../src/lib/shell-version.txt").trim()
}

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/DRincs-Productions/roves/releases/latest";

/// True only when *this exact build of Packmaster* was itself produced by `roves-packmaster`'s own
/// `test.yml` (which sets `PACKMASTER_TEST_BUILD=1` before `npm run tauri build` — see that
/// workflow), never by inspecting anything at runtime: a shipped, tagged Packmaster release
/// must behave identically regardless of whatever's in an end user's own environment.
pub fn is_test_build() -> bool {
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
/// Steam build `..._steam.zip`, or the local-MCP development build `..._mcp.zip` (published
/// for every desktop platform). The engine publishes no Steam + MCP package, so that
/// combination is an error, never a silent fallback.
pub fn asset_name(platform: Platform, steam: bool, mcp: bool) -> Result<String, String> {
    let suffix = match (steam, mcp) {
        (false, false) => "",
        (true, false) => "_steam",
        (false, true) => "_mcp",
        (true, true) => {
            return Err(
                "the local MCP bridge and Steam can't be combined: the MCP-enabled players have no Steam support"
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

/// The sidecar's `ROVES_CACHE_DIR`: this app's cache folder. roves-cli keeps the player zips in
/// `<it>/shells/<version>/<asset>/` (with a `.complete` marker), the same layout Packmaster's
/// own downloader used, so existing caches stay valid.
pub fn cache_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_cache_dir().map_err(|e| e.to_string())
}

fn shells_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(cache_root(app)?.join("shells"))
}

/// Total on-disk size of every cached player package, across every version/asset ever
/// downloaded.
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
/// generation just downloads fresh. Not scoped to "only the stale/unused ones": simplest
/// correct behavior, and a stale version is never served anyway (the cache is keyed by
/// version) -- this only reclaims disk space.
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
            assert_eq!(asset_name(platform, false, true).unwrap(), format!("roves_shell_{id}_mcp.zip"));
            assert!(asset_name(platform, true, true).unwrap_err().contains("Steam"));
        }
    }

    #[test]
    fn asset_url_uses_the_release_download_scheme() {
        assert_eq!(
            asset_url("roves_shell_linux_steam.zip", "v0.9.0"),
            "https://github.com/DRincs-Productions/roves/releases/download/v0.9.0/roves_shell_linux_steam.zip"
        );
    }

    #[test]
    fn platforms_round_trip_their_ids() {
        for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
            assert_eq!(Platform::from_id(platform.id()), Some(platform));
        }
        assert_eq!(Platform::from_id("android"), None);
    }
}
