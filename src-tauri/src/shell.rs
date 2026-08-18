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

/// `windows` | `macos` | `linux` — matches release.yml's own `matrix.os_name` and the
/// `roves_shell_<os_name>.zip`/`roves_shell_<os_name>_steam.zip` asset naming (the engine's
/// own `matrix.asset_suffix`).
pub fn shell_asset_url(platform: &str, steam: bool) -> String {
    let suffix = if steam { "_steam" } else { "" };
    format!(
        "https://github.com/DRincs-Productions/roves/releases/download/{TARGET_SHELL_VERSION}/roves_shell_{platform}{suffix}.zip"
    )
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
        .join(TARGET_SHELL_VERSION)
        .join(platform)
        // Plain and Steam-enabled shells are different binaries entirely -- keying the cache
        // by variant too means toggling Steam on/off never reuses (or clobbers) the wrong
        // cached extraction.
        .join(if steam { "steam" } else { "plain" });
    let extracted_root = cache_dir.join("roves");
    let marker = cache_dir.join(".complete");
    if marker.exists() && extracted_root.exists() {
        on_progress(1.0);
        return Ok(extracted_root);
    }

    if cache_dir.exists() {
        tokio::fs::remove_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| e.to_string())?;

    let zip_path = cache_dir.join("shell.zip");
    download_file(&shell_asset_url(platform, steam), &zip_path, &mut on_progress).await?;
    extract_zip(&zip_path, &cache_dir)?;
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
