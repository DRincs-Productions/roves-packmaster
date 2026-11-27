//! Desktop packaging through the roves-cli sidecar: `@drincs/roves-cli` built as a standalone
//! executable (Node.js Single Executable Application, no Node.js needed on the user's machine)
//! and bundled with Packmaster as Tauri `externalBin` (`binaries/roves-<target-triple>[.exe]`,
//! copied next to Packmaster's executable as `roves[.exe]`). Its version is pinned in
//! src/lib/roves-cli-version.txt (`scripts/fetch-roves-cli.mjs` downloads that release's binary).
//!
//! The CLI downloads (and caches) the engine's player package, verifies it, adds the game,
//! applies the project icon, Steam and MCP rules, and writes the zip (and, for installers, the
//! extracted folder) -- the same code roves-action runs. Packmaster passes `--json` (the result
//! on stdout) and `--progress json` (one JSON progress line per event on stderr; any other
//! stderr line is a log line, and `roves: error: ...` the failure).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

/// One `roves bundle` run for a single platform.
pub struct BundleRequest<'a> {
    /// `windows` | `macos` | `linux`.
    pub platform: &'a str,
    pub content_dir: &'a Path,
    /// Folder (or `<name>.app`) inside the zip, and the zip's base name.
    pub name: &'a str,
    pub out_dir: &'a Path,
    pub shell_version: &'a str,
    /// `Some(app id)`: the Steam variant, with `steam_appid.txt` next to the executable.
    pub steam_app_id: Option<&'a str>,
    pub mcp: bool,
    /// Explicit project icon (the Release info icon).
    pub icon_png: Option<&'a Path>,
    /// Also write the unzipped package (needed by the installers).
    pub extract: bool,
    /// Download the player again even if cached (test builds follow the latest release).
    pub refresh: bool,
    /// `ROVES_CACHE_DIR` of the sidecar.
    pub cache_dir: &'a Path,
    /// `files` forces loose files; `archive` (the CLI default) is not passed, so the CLI can
    /// fall back to loose files for a player older than v0.9.1.
    pub content_layout: &'a str,
    pub compression_level: u8,
    pub max_archive_size_mb: u32,
}

/// The fields of roves-cli's `--json` result that Packmaster uses.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BundleResult {
    pub archive_path: PathBuf,
    pub bundle_dir: Option<PathBuf>,
    pub top_folder: String,
    /// The player executable, relative to the package root (for the `.deb`).
    pub executable: String,
    pub icon_source: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// A `--progress json` line: `{"type":"progress","platform":..,"stage":..,"fraction":..}`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Progress {
    pub stage: String,
    pub fraction: f64,
}

/// The sidecar next to Packmaster's own executable (where Tauri puts `externalBin`, also in
/// `tauri dev`).
pub fn sidecar_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or_else(|| "executable has no parent directory".to_string())?;
    let sidecar = dir.join(format!("roves{}", std::env::consts::EXE_SUFFIX));
    if sidecar.is_file() {
        Ok(sidecar)
    } else {
        Err(format!("the roves-cli sidecar is missing ({})", sidecar.display()))
    }
}

pub fn bundle_args(request: &BundleRequest) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["bundle", "--json", "--progress", "json", "--platform", request.platform]
        .into_iter()
        .map(OsString::from)
        .collect();
    let mut push = |flag: &str, value: &dyn AsRef<std::ffi::OsStr>| {
        args.push(flag.into());
        args.push(value.as_ref().to_os_string());
    };
    push("--content-dir", &request.content_dir);
    push("--name", &request.name);
    push("--out-dir", &request.out_dir);
    push("--shell-version", &request.shell_version);
    if let Some(app_id) = request.steam_app_id {
        args.push("--steam".into());
        args.push("--steam-app-id".into());
        args.push(app_id.into());
    }
    if request.mcp {
        args.push("--enable-mcp".into());
    }
    if let Some(icon) = request.icon_png {
        args.push("--icon-png".into());
        args.push(icon.into());
    }
    if !request.extract {
        args.push("--no-extract".into());
    }
    if request.refresh {
        args.push("--refresh".into());
    }
    if request.content_layout == "files" {
        args.push("--content-layout".into());
        args.push("files".into());
    }
    args.push("--compression-level".into());
    args.push(request.compression_level.to_string().into());
    args.push("--max-archive-size".into());
    args.push(format!("{}MB", request.max_archive_size_mb).into());
    args
}

/// The progress event of a stderr line, if it is one.
pub fn parse_progress(line: &str) -> Option<Progress> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "progress" {
        return None;
    }
    serde_json::from_value(value).ok()
}

/// The single platform result of `roves bundle --json` (an array with one entry).
pub fn parse_result(stdout: &str) -> Result<BundleResult, String> {
    let mut results: Vec<BundleResult> =
        serde_json::from_str(stdout.trim()).map_err(|e| format!("unexpected roves-cli output: {e}"))?;
    if results.len() != 1 {
        return Err(format!("roves-cli returned {} results instead of one", results.len()));
    }
    Ok(results.remove(0))
}

/// The user-facing failure from the CLI's stderr: its `roves: error: ...` message, else the
/// last lines it printed.
pub fn failure_message(log: &[String], status: &str) -> String {
    if let Some(at) = log.iter().position(|line| line.starts_with("roves: error: ")) {
        let mut message = log[at].trim_start_matches("roves: error: ").to_string();
        for line in &log[at + 1..] {
            message.push('\n');
            message.push_str(line);
        }
        return message;
    }
    let tail: Vec<&str> = log.iter().rev().take(10).rev().map(String::as_str).collect();
    format!("roves-cli failed ({status}){}{}", if tail.is_empty() { "" } else { ":\n" }, tail.join("\n"))
}

/// Runs `roves bundle` with `sidecar`, reporting each progress event to `on_progress(stage,
/// fraction)`.
pub async fn bundle_with(
    sidecar: &Path,
    request: &BundleRequest<'_>,
    mut on_progress: impl FnMut(&str, f64) + Send,
) -> Result<BundleResult, String> {
    let mut command = tokio::process::Command::new(sidecar);
    command
        .args(bundle_args(request))
        .env("ROVES_CACHE_DIR", request.cache_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // No console window flashing up behind Packmaster's own window.
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let mut child = command.spawn().map_err(|e| format!("running {}: {e}", sidecar.display()))?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;

    let read_stdout = async {
        let mut text = String::new();
        stdout.read_to_string(&mut text).await.map(|_| text)
    };
    let read_stderr = async {
        let mut log = Vec::new();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match parse_progress(&line) {
                Some(progress) => on_progress(&progress.stage, progress.fraction),
                None => {
                    eprintln!("roves: {line}");
                    log.push(line);
                },
            }
        }
        log
    };
    let (stdout, log) = futures_util::future::join(read_stdout, read_stderr).await;
    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(failure_message(&log, &status.to_string()));
    }
    parse_result(&stdout.map_err(|e| e.to_string())?)
}

/// `bundle_with` the bundled sidecar.
pub async fn bundle(
    request: &BundleRequest<'_>,
    on_progress: impl FnMut(&str, f64) + Send,
) -> Result<BundleResult, String> {
    bundle_with(&sidecar_path()?, request, on_progress).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(content: &'a Path, out: &'a Path, cache: &'a Path) -> BundleRequest<'a> {
        BundleRequest {
            platform: "windows",
            content_dir: content,
            name: "my_game",
            out_dir: out,
            shell_version: "v0.9.0",
            steam_app_id: None,
            mcp: false,
            icon_png: None,
            extract: false,
            refresh: false,
            cache_dir: cache,
            content_layout: "archive",
            compression_level: 0,
            max_archive_size_mb: 300,
        }
    }

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn arguments_cover_every_option() {
        let (content, out, cache) = (Path::new("dist"), Path::new("out"), Path::new("cache"));
        assert_eq!(
            strings(bundle_args(&request(content, out, cache))),
            [
                "bundle", "--json", "--progress", "json", "--platform", "windows", "--content-dir", "dist", "--name",
                "my_game", "--out-dir", "out", "--shell-version", "v0.9.0", "--no-extract",
                "--compression-level", "0", "--max-archive-size", "300MB"
            ]
        );
        let icon = Path::new("icon.png");
        let full = BundleRequest {
            steam_app_id: Some("480"),
            icon_png: Some(icon),
            extract: true,
            refresh: true,
            ..request(content, out, cache)
        };
        let args = strings(bundle_args(&full));
        assert!(args.ends_with(&[
            "--steam".to_string(),
            "--steam-app-id".into(),
            "480".into(),
            "--icon-png".into(),
            "icon.png".into(),
            "--refresh".into(),
            "--compression-level".into(),
            "0".into(),
            "--max-archive-size".into(),
            "300MB".into()
        ]));
        let loose = BundleRequest {
            content_layout: "files",
            compression_level: 6,
            max_archive_size_mb: 1000,
            ..request(content, out, cache)
        };
        assert!(strings(bundle_args(&loose)).ends_with(&[
            "--content-layout".to_string(),
            "files".into(),
            "--compression-level".into(),
            "6".into(),
            "--max-archive-size".into(),
            "1000MB".into()
        ]));
        let mcp = BundleRequest { mcp: true, ..request(content, out, cache) };
        assert!(strings(bundle_args(&mcp)).contains(&"--enable-mcp".to_string()));
    }

    #[test]
    fn progress_lines_are_told_apart_from_logs() {
        assert_eq!(
            parse_progress(r#"{"type":"progress","platform":"windows","stage":"downloading","fraction":0.25}"#),
            Some(Progress { stage: "downloading".into(), fraction: 0.25 })
        );
        assert_eq!(parse_progress("windows: wrote out.zip"), None);
        assert_eq!(parse_progress(r#"{"type":"other"}"#), None);
    }

    #[test]
    fn the_json_result_and_errors_are_read() {
        let stdout = r#"[{"platform":"linux","archivePath":"/o/g.zip","bundleDir":"/o/g","topFolder":"g",
            "executable":"roves-cef-linux-gate","iconSource":"window roves (x)","mcpEnabled":false,"warnings":[]}]"#;
        let result = parse_result(stdout).unwrap();
        assert_eq!(result.archive_path, PathBuf::from("/o/g.zip"));
        assert_eq!(result.bundle_dir, Some(PathBuf::from("/o/g")));
        assert_eq!(result.executable, "roves-cef-linux-gate");
        assert!(parse_result("[]").is_err());
        let log = vec!["windows: base checked".to_string(), "roves: error: content dir x has no index.html".to_string()];
        assert_eq!(failure_message(&log, "exit code: 1"), "content dir x has no index.html");
        assert!(failure_message(&["boom".to_string()], "exit code: 2").contains("boom"));
    }

    /// End to end against a real sidecar and player zip, like the smoke test of the CLI:
    /// `ROVES_CLI_SIDECAR=<roves exe> ROVES_SMOKE_SHELL_ZIP=<roves_shell_windows.zip>
    /// ROVES_SMOKE_GAME=<game build> cargo test --lib real_sidecar -- --ignored --nocapture`.
    /// The local player zip goes in the cache layout roves-cli reads, so nothing is downloaded.
    #[test]
    #[ignore = "needs ROVES_CLI_SIDECAR, ROVES_SMOKE_SHELL_ZIP and ROVES_SMOKE_GAME"]
    fn real_sidecar() {
        let var = |name: &str| PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")));
        let (sidecar, shell_zip, game) = (var("ROVES_CLI_SIDECAR"), var("ROVES_SMOKE_SHELL_ZIP"), var("ROVES_SMOKE_GAME"));
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        let asset_dir = cache.join("shells").join("v0.0.0-smoke").join("roves_shell_windows");
        std::fs::create_dir_all(&asset_dir).unwrap();
        std::fs::copy(&shell_zip, asset_dir.join("roves_shell_windows.zip")).unwrap();
        std::fs::write(asset_dir.join(".complete"), "1").unwrap();
        let out = tmp.path().join("out");
        let request = BundleRequest {
            shell_version: "v0.0.0-smoke",
            extract: true,
            ..request(&game, &out, &cache)
        };
        let mut stages = Vec::new();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let result = runtime
            .block_on(bundle_with(&sidecar, &request, |stage, fraction| stages.push((stage.to_string(), fraction))))
            .unwrap();
        eprintln!("{result:?}\nstages: {stages:?}");
        assert!(result.archive_path.is_file());
        assert_eq!(result.top_folder, "my_game");
        assert_eq!(result.executable, "play.exe");
        assert!(result.bundle_dir.unwrap().join("game").join("index.html").is_file());
        assert!(stages.iter().any(|(stage, _)| stage == "assembling"));
    }
}
