//! Experimental CEF Windows player package (engine branch `codex/cef-sdl3-wgpu-probe`,
//! `support/cef-netrender-probe`): unlike every other desktop output, nothing is downloaded
//! here. The base is a player package the engine CI uploaded as a workflow artifact
//! (`visual-novel-template-roves-cef-windows-x64`, or the MCP-enabled
//! `visual-novel-template-roves-cef-mcp-windows-x64`), downloaded and extracted by the user.
//! There is no published release asset for it yet, so there is no URL to pin.
//!
//! The package keeps the base's `play.exe`/`play.dll`/CEF runtime untouched and replaces
//! `game\` with the user's build output. The local MCP bridge of that player turns on only
//! when the package contains the marker `mcp\enabled` (engine `support/cef-netrender-probe/
//! mcp/README.md`), so:
//!
//! - **MCP off (default)**: the output never contains `mcp\` (marker, `server.mjs`, its npm
//!   dependencies) nor the MCP provenance, even when the base is an MCP-enabled package.
//! - **MCP on**: only accepted when the base itself is a complete MCP-enabled package (every
//!   file listed below, plus the MCP section the engine CI writes into `PROVENANCE.txt`).
//!   Packmaster never adds MCP files of its own; a base without them is an error, not a
//!   package that silently runs without the bridge.
//!
//! **Project icon** (engine `support/cef-netrender-probe/README.md`, "Project icon"; the same
//! rules as the engine's and roves-action's `project_icon.py`, keep the three in step): the
//! game's icon comes first, the official Roves icon is the fallback. The window icon is an
//! `icon.png` next to `play.exe` (the Release info icon, else `<build>/icon.png`, else the
//! largest frame of `<build>/icon.ico`/`favicon.ico`; none: the player shows the Roves logo);
//! `play.exe`'s icon resource gets an ICO generated from that PNG, else the game's ICO, else
//! the base's `roves.ico` (bases without it keep their own `play.exe` icon). The base's own
//! `icon.png` belongs to its sample game and is never copied.
//!
//! Every base is checked against its own `SHA256SUMS.txt` first (every file listed, every
//! listed file present with that hash, nothing unlisted), so a modified or partially extracted
//! artifact is rejected. That proves the files are the ones the CI packaged, not that the CI
//! run that produced them was green: picking an artifact from a successful run stays the
//! user's job (Packmaster has no GitHub credentials to check it).

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

const CHECKSUMS: &str = "SHA256SUMS.txt";
/// Window icon next to `play.exe` (read by the player at startup).
const PROJECT_ICON_FILE: &str = "icon.png";
/// The official Roves icon the engine CI ships in the base, the `play.exe` fallback.
const ROVES_FALLBACK_ICO: &str = "roves.ico";
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;
const MIN_ICON_PX: u32 = 16;
const MAX_ICON_PX: u32 = 2048;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
/// Frames of a generated `play.exe` icon (those not larger than the source).
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const PROVENANCE: &str = "PROVENANCE.txt";
const GAME_DIR: &str = "game";
const MCP_DIR: &str = "mcp";
/// Written by the engine CI's "assemble the MCP-enabled package" step, at the end of
/// `PROVENANCE.txt`.
const MCP_PROVENANCE_HEADER: &str = "MCP-enabled development package";

const BASE_REQUIRED: &[&str] = &[
    "play.exe",
    "play.dll",
    "play.exe.manifest",
    "libcef.dll",
    PROVENANCE,
    CHECKSUMS,
];
const MCP_REQUIRED: &[&str] = &[
    "mcp/enabled",
    "mcp/server.mjs",
    "mcp/README.md",
    "mcp/node_modules/@modelcontextprotocol/server/package.json",
];

#[derive(Debug)]
pub struct BasePackage {
    pub root: PathBuf,
    /// Complete, checksum-verified MCP-enabled package.
    pub mcp_capable: bool,
}

/// The folder holding `play.exe`: `dir` itself, or its only subfolder that has one (a
/// workflow artifact extracts as `<artifact>/<package folder>/play.exe`).
pub fn locate_base(dir: &Path) -> Result<PathBuf, String> {
    if dir.join("play.exe").is_file() {
        return Ok(dir.to_path_buf());
    }
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() && path.join("play.exe").is_file() {
            found.push(path);
        }
    }
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(format!(
            "{} is not a Roves CEF player package (no play.exe in it or in one of its subfolders)",
            dir.display()
        )),
        _ => Err(format!(
            "{} contains more than one CEF player package; pick one",
            dir.display()
        )),
    }
}

/// Required files, `SHA256SUMS.txt` verification, MCP capability.
pub fn inspect_base(root: &Path) -> Result<BasePackage, String> {
    for required in BASE_REQUIRED {
        if !root.join(required).is_file() {
            return Err(format!("the CEF base package lacks {required}"));
        }
    }
    verify_checksums(root)?;
    let provenance = std::fs::read_to_string(root.join(PROVENANCE)).map_err(|e| e.to_string())?;
    let mcp_capable = MCP_REQUIRED.iter().all(|file| root.join(file).is_file())
        && provenance
            .lines()
            .any(|line| line.starts_with(MCP_PROVENANCE_HEADER));
    Ok(BasePackage {
        root: root.to_path_buf(),
        mcp_capable,
    })
}

fn relative_slash(root: &Path, path: &Path) -> Result<String, String> {
    let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn verify_checksums(root: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(root.join(CHECKSUMS)).map_err(|e| e.to_string())?;
    let mut listed = std::collections::BTreeMap::new();
    for line in text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let (hash, path) = line
            .split_once("  ")
            .ok_or_else(|| format!("malformed {CHECKSUMS} line: {line:?}"))?;
        let path = path.trim_end();
        if Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!("{CHECKSUMS} lists an unsafe path: {path:?}"));
        }
        listed.insert(path.to_string(), hash.to_ascii_lowercase());
    }
    let mut actual = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_file() {
            let rel = relative_slash(root, entry.path())?;
            if rel != CHECKSUMS {
                actual.push(rel);
            }
        }
    }
    if let Some(extra) = actual.iter().find(|rel| !listed.contains_key(*rel)) {
        return Err(format!(
            "the CEF base package contains {extra}, which its {CHECKSUMS} does not list"
        ));
    }
    for (rel, hash) in &listed {
        let path = root.join(rel);
        if !path.is_file() {
            return Err(format!(
                "the CEF base package lacks {rel}, listed in its {CHECKSUMS}"
            ));
        }
        if &sha256_hex(&path)? != hash {
            return Err(format!(
                "{rel} does not match the CEF base package's {CHECKSUMS}"
            ));
        }
    }
    Ok(())
}

/// Whether a base-relative path (forward slashes) is left out of the output.
fn excluded(rel: &str, enable_mcp: bool) -> bool {
    let first = rel.split('/').next().unwrap_or("");
    first == GAME_DIR
        || rel == CHECKSUMS
        || rel == PROVENANCE
        // The base's sample game's window icon; the output gets the resolved one.
        || rel == PROJECT_ICON_FILE
        // Provenance of the base's own sample game, which the output no longer contains.
        || rel.starts_with("provenance/visual-novel-template-")
        || (!enable_mcp && (first == MCP_DIR || rel.starts_with("provenance/mcp-")))
}

/// Copies `base` to `dest` (which must not exist) with `game\` replaced by `content_dir`,
/// applies the MCP choice and the resolved project icon (`patch_exe(play.exe, ico)` patches
/// the executable's icon resource; rcedit in `bundle.rs`), rewrites `PROVENANCE.txt` and
/// regenerates `SHA256SUMS.txt`.
pub fn assemble(
    base: &BasePackage,
    content_dir: &Path,
    dest: &Path,
    enable_mcp: bool,
    icon: &ProjectIcon,
    patch_exe: &dyn Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    if enable_mcp && !base.mcp_capable {
        return Err(
            "the local MCP bridge is enabled, but the selected CEF base package is not a complete MCP-enabled package \
             (download the visual-novel-template-roves-cef-mcp-windows-x64 artifact of a successful engine run, or turn MCP off)"
                .to_string(),
        );
    }
    if !content_dir.join("index.html").is_file() {
        return Err(format!(
            "{} has no index.html at its root",
            content_dir.display()
        ));
    }
    if dest.exists() {
        return Err(format!("{} already exists", dest.display()));
    }

    let root = &base.root;
    let walker = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            relative_slash(root, entry.path())
                .map_or(true, |rel| rel.is_empty() || !excluded(&rel, enable_mcp))
        });
    for entry in walker {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = dest.join(entry.path().strip_prefix(root).map_err(|e| e.to_string())?);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            std::fs::copy(entry.path(), &target)
                .map_err(|e| format!("{}: {e}", entry.path().display()))?;
        }
    }
    for entry in walkdir::WalkDir::new(content_dir) {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = dest.join(GAME_DIR).join(
            entry
                .path()
                .strip_prefix(content_dir)
                .map_err(|e| e.to_string())?,
        );
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            std::fs::copy(entry.path(), &target)
                .map_err(|e| format!("{}: {e}", entry.path().display()))?;
        }
    }

    if let Some(png) = &icon.window_png {
        std::fs::copy(png, dest.join(PROJECT_ICON_FILE))
            .map_err(|e| format!("{}: {e}", png.display()))?;
    }
    if let Some(ico) = &icon.exe_ico {
        patch_exe(&dest.join("play.exe"), ico)?;
    }

    let provenance = std::fs::read_to_string(root.join(PROVENANCE)).map_err(|e| e.to_string())?;
    std::fs::write(
        dest.join(PROVENANCE),
        rewrite_provenance(&provenance, enable_mcp, icon),
    )
    .map_err(|e| e.to_string())?;
    write_checksums(dest)?;

    let marker = dest.join(MCP_DIR).join("enabled");
    if enable_mcp != marker.is_file() || (!enable_mcp && dest.join(MCP_DIR).exists()) {
        return Err(
            "internal error: the MCP state of the assembled package does not match the setting"
                .to_string(),
        );
    }
    Ok(())
}

fn rewrite_provenance(base: &str, enable_mcp: bool, icon: &ProjectIcon) -> String {
    let mut lines: Vec<&str> = base.trim_start_matches('\u{feff}').lines().collect();
    if !enable_mcp {
        if let Some(at) = lines
            .iter()
            .position(|line| line.starts_with(MCP_PROVENANCE_HEADER))
        {
            lines.truncate(at);
        }
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    let mut out = lines.join("\r\n");
    out.push_str(
        "\r\n\r\nGame replaced by Roves Packmaster\r\n\
         \x20 game\\ contains the developer's own build output, not the sample game described in\r\n\
         \x20 the \"Game\" section above; the sample game's provenance files were removed. The\r\n",
    );
    out.push_str(if icon.exe_ico.is_some() {
        "  engine files (play.exe, play.dll, CEF runtime) are the base package's; only play.exe's\r\n\
         \x20 icon resource was replaced (electron/rcedit v2.0.0).\r\n"
    } else {
        "  engine files (play.exe, play.dll, CEF runtime) are the base package's, unmodified.\r\n"
    });
    if !icon.summary.is_empty() {
        out.push_str(&format!("  icons: {}\r\n", icon.summary));
    }
    out.push_str(if enable_mcp {
        "  Local MCP bridge: ON (mcp\\enabled kept). Development and playtests only: any program\r\n\
         \x20 running as your Windows user can read and control the running game. Never ship to players.\r\n"
    } else {
        "  Local MCP bridge: OFF (no mcp\\ folder, no activation marker).\r\n"
    });
    out
}

fn write_checksums(dest: &Path) -> Result<(), String> {
    let mut lines = Vec::new();
    for entry in walkdir::WalkDir::new(dest).sort_by_file_name() {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_file() {
            let rel = relative_slash(dest, entry.path())?;
            if rel != CHECKSUMS {
                lines.push((rel.clone(), format!("{}  {rel}", sha256_hex(entry.path())?)));
            }
        }
    }
    lines.sort();
    let mut file = std::fs::File::create(dest.join(CHECKSUMS)).map_err(|e| e.to_string())?;
    for (_, line) in lines {
        write!(file, "{line}\r\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Resolved project icon (see the module documentation).
#[derive(Debug, Default)]
pub struct ProjectIcon {
    /// Copied as `icon.png` next to `play.exe`; `None`: the player shows the Roves logo.
    pub window_png: Option<PathBuf>,
    /// Patched into `play.exe`; `None`: `play.exe` keeps the base's icon.
    pub exe_ico: Option<PathBuf>,
    /// `window <project|roves> (...); play.exe <project|roves|unchanged> (...)`.
    pub summary: String,
    /// Invalid icon files found in the build output and skipped.
    pub warnings: Vec<String>,
}

fn read_icon_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("not a file".to_string());
    }
    if metadata.len() > MAX_ICON_BYTES {
        return Err(format!(
            "{} bytes exceeds the {MAX_ICON_BYTES}-byte limit",
            metadata.len()
        ));
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

fn check_edge(width: u32, height: u32) -> Result<(), String> {
    if width != height {
        return Err(format!("{width}x{height} is not square"));
    }
    if !(MIN_ICON_PX..=MAX_ICON_PX).contains(&width) {
        return Err(format!(
            "{width}x{height} is outside {MIN_ICON_PX}..={MAX_ICON_PX} px"
        ));
    }
    Ok(())
}

/// Edge of a valid project PNG: at most 8 MiB, PNG signature with `IHDR` first, square
/// 16..=2048 px, fully decodable with the header's size.
fn check_png(path: &Path) -> Result<u32, String> {
    let data = read_icon_file(path)?;
    if data.len() < 33 || &data[..8] != PNG_SIGNATURE || data[8..16] != *b"\0\0\0\x0dIHDR" {
        return Err("not a PNG file (signature or IHDR missing)".to_string());
    }
    let be = |at: usize| u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
    let (width, height) = (be(16), be(20));
    check_edge(width, height)?;
    let image = image::load_from_memory_with_format(&data, image::ImageFormat::Png)
        .map_err(|e| format!("decode: {e}"))?;
    if (image.width(), image.height()) != (width, height) {
        return Err(format!(
            "decoded as {}x{}",
            image.width(),
            image.height()
        ));
    }
    Ok(width)
}

/// Every frame of an ICO, each decoded on its own (as a one-entry icon directory).
fn ico_frames(data: &[u8]) -> Result<Vec<image::DynamicImage>, String> {
    let le16 = |at: usize| u16::from_le_bytes([data[at], data[at + 1]]);
    let le32 = |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
    if data.len() < 22 || data[..4] != [0, 0, 1, 0] || le16(4) == 0 {
        return Err("not an ICO file (icon directory header missing)".to_string());
    }
    let count = usize::from(le16(4));
    let directory_end = 6 + count * 16;
    if directory_end > data.len() {
        return Err("icon directory truncated".to_string());
    }
    (0..count)
        .map(|i| {
            let entry = 6 + i * 16;
            let (size, offset) = (le32(entry + 8) as usize, le32(entry + 12) as usize);
            let end = offset.checked_add(size).filter(|&end| end <= data.len());
            let Some(end) = end.filter(|_| offset >= directory_end && size > 0) else {
                return Err(format!("frame {i} lies outside the file"));
            };
            let mut single = Vec::with_capacity(22 + size);
            single.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
            single.extend_from_slice(&data[entry..entry + 12]);
            single.extend_from_slice(&22u32.to_le_bytes());
            single.extend_from_slice(&data[offset..end]);
            image::load_from_memory_with_format(&single, image::ImageFormat::Ico)
                .map_err(|e| format!("decode frame {i}: {e}"))
        })
        .collect()
}

fn largest_frame(frames: Vec<image::DynamicImage>) -> Option<image::DynamicImage> {
    frames
        .into_iter()
        .max_by_key(|frame| u64::from(frame.width()) * u64::from(frame.height()))
}

/// Largest frame edge of a valid project ICO: at most 8 MiB, every frame decodable, the
/// largest frame a square of at least 16 px.
fn check_ico(path: &Path) -> Result<u32, String> {
    let data = read_icon_file(path)?;
    let largest = largest_frame(ico_frames(&data)?).ok_or("no frame")?;
    let (width, height) = (largest.width(), largest.height());
    if width != height || width < MIN_ICON_PX {
        return Err(format!(
            "largest frame {width}x{height} is not a square of at least {MIN_ICON_PX} px"
        ));
    }
    Ok(width)
}

/// First valid candidate `(path, label, edge)`. An explicit candidate that breaks the rules
/// is an error; a file found in the build output is a warning and the next one is tried.
fn pick_icon(
    candidates: &[(PathBuf, &str, bool)],
    check: fn(&Path) -> Result<u32, String>,
    warnings: &mut Vec<String>,
) -> Result<Option<(PathBuf, String, u32)>, String> {
    for (path, label, explicit) in candidates {
        if !explicit && !path.is_file() {
            continue;
        }
        match check(path) {
            Ok(edge) => return Ok(Some((path.clone(), label.to_string(), edge))),
            Err(error) if *explicit => return Err(format!("{label} {}: {error}", path.display())),
            Err(error) => warnings.push(format!("ignoring {label} ({error}); trying the next source")),
        }
    }
    Ok(None)
}

/// Resolves the project icon for a package assembled from `base_root`, writing generated
/// files into `work_dir` (created if needed). `explicit_png` is Packmaster's Release info
/// icon.
pub fn resolve_project_icon(
    explicit_png: Option<&Path>,
    content_dir: &Path,
    base_root: &Path,
    work_dir: &Path,
) -> Result<ProjectIcon, String> {
    std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let mut warnings = Vec::new();
    let mut png_candidates = Vec::new();
    if let Some(path) = explicit_png {
        png_candidates.push((path.to_path_buf(), "Release info icon", true));
    }
    png_candidates.push((content_dir.join(PROJECT_ICON_FILE), "game icon.png", false));
    let png = pick_icon(&png_candidates, check_png, &mut warnings)?;
    let ico = pick_icon(
        &[
            (content_dir.join("icon.ico"), "game icon.ico", false),
            (content_dir.join("favicon.ico"), "game favicon.ico", false),
        ],
        check_ico,
        &mut warnings,
    )?;

    let (window_png, window) = if let Some((path, label, edge)) = &png {
        (Some(path.clone()), format!("project ({label} {edge}x{edge})"))
    } else if let Some((path, label, edge)) = &ico {
        let data = read_icon_file(path)?;
        let frame = largest_frame(ico_frames(&data)?).ok_or("no frame")?;
        let out = work_dir.join(PROJECT_ICON_FILE);
        frame
            .to_rgba8()
            .save_with_format(&out, image::ImageFormat::Png)
            .map_err(|e| format!("writing {}: {e}", out.display()))?;
        check_png(&out)?;
        (Some(out), format!("project (largest frame of {label} {edge}x{edge})"))
    } else {
        (None, "roves (no project icon: embedded Roves logo)".to_string())
    };

    let fallback = base_root.join(ROVES_FALLBACK_ICO);
    let (exe_ico, exe) = if let Some((path, label, edge)) = &png {
        let out = work_dir.join("play.ico");
        let sizes: Vec<u32> = ICO_SIZES.into_iter().filter(|&size| size <= *edge).collect();
        crate::icon::generate_ico_with_sizes(path, &out, &sizes)?;
        check_ico(&out)?;
        (Some(out), format!("project (ICO generated from {label})"))
    } else if let Some((path, label, _)) = &ico {
        (Some(path.clone()), format!("project ({label})"))
    } else if fallback.is_file() {
        check_ico(&fallback).map_err(|e| format!("the base's {ROVES_FALLBACK_ICO}: {e}"))?;
        (Some(fallback), "roves (Roves fallback icon)".to_string())
    } else {
        (None, "unchanged (no project icon and no Roves fallback icon in the base)".to_string())
    };

    Ok(ProjectIcon {
        window_png,
        exe_ico,
        summary: format!("window {window}; play.exe {exe}"),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    /// A miniature CI package (same layout and checksum format as the engine workflow).
    fn fake_base(root: &Path, mcp: bool) {
        for file in [
            "play.exe",
            "play.dll",
            "play.exe.manifest",
            "libcef.dll",
            "locales/en-US.pak",
        ] {
            write(root, file, file.as_bytes());
        }
        write(root, "game/index.html", b"<p>sample</p>");
        write(root, "game/assets/sample.js", b"sample");
        write(root, "provenance/Cargo.lock", b"lock");
        write(
            root,
            "provenance/visual-novel-template-npm-ls.txt",
            b"template",
        );
        let mut provenance =
            String::from("Engine\r\n  commit abc\r\n\r\nGame (folder game\\)\r\n  template\r\n");
        if mcp {
            for file in MCP_REQUIRED {
                write(root, file, b"mcp");
            }
            write(root, "provenance/mcp-npm-ls.txt", b"mcp deps");
            provenance.push_str("\r\nMCP-enabled development package (enable-mcp=true; never ship to players)\r\n  mcp\\enabled\r\n");
        }
        write(root, PROVENANCE, provenance.as_bytes());
        write_checksums(root).unwrap();
    }

    fn files(root: &Path) -> Vec<String> {
        let mut out: Vec<String> = walkdir::WalkDir::new(root)
            .into_iter()
            .map(Result::unwrap)
            .filter(|e| e.file_type().is_file())
            .map(|e| relative_slash(root, e.path()).unwrap())
            .collect();
        out.sort();
        out
    }

    fn no_patch(_: &Path, _: &Path) -> Result<(), String> {
        panic!("play.exe must not be patched without a resolved icon")
    }

    fn png(path: &Path, edge: u32, rgba: [u8; 4]) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbaImage::from_pixel(edge, edge, image::Rgba(rgba))
            .save_with_format(path, image::ImageFormat::Png)
            .unwrap();
        path.to_path_buf()
    }

    fn ico(path: &Path, edge: u32, rgba: [u8; 4]) -> PathBuf {
        let source = path.with_extension("source.png");
        png(&source, edge, rgba);
        crate::icon::generate_ico_with_sizes(&source, path, &[16, edge]).unwrap();
        std::fs::remove_file(source).unwrap();
        path.to_path_buf()
    }

    fn first_pixel(path: &Path) -> [u8; 4] {
        image::open(path).unwrap().to_rgba8().get_pixel(0, 0).0
    }

    /// A base as the engine CI now ships it: the sample game's `icon.png` and `roves.ico`.
    fn icon_base(root: &Path, with_fallback: bool) -> BasePackage {
        fake_base(root, false);
        png(&root.join(PROJECT_ICON_FILE), 64, [0, 0, 255, 255]);
        if with_fallback {
            ico(&root.join(ROVES_FALLBACK_ICO), 256, [20, 20, 24, 255]);
        }
        write_checksums(root).unwrap();
        inspect_base(root).unwrap()
    }

    /// Assembles with a recording `patch_exe`; returns the ICO it was called with.
    fn assemble_with_icon(base: &BasePackage, content: &Path, out: &Path, icon: &ProjectIcon) -> Option<PathBuf> {
        let patched = std::cell::RefCell::new(None);
        assemble(base, content, out, false, icon, &|exe, ico| {
            assert_eq!(exe, out.join("play.exe"));
            *patched.borrow_mut() = Some(ico.to_path_buf());
            Ok(())
        })
        .unwrap();
        verify_checksums(out).unwrap();
        patched.into_inner()
    }

    #[test]
    fn the_game_icon_png_is_used_for_window_and_play_exe() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, out, work) = (tmp.path().join("dist"), tmp.path().join("out"), tmp.path().join("work"));
        let base = icon_base(&tmp.path().join("pkg"), true);
        dist(&content);
        png(&content.join("icon.png"), 512, [200, 30, 30, 255]);
        let icon = resolve_project_icon(None, &content, &base.root, &work).unwrap();
        assert!(icon.summary.starts_with("window project (game icon.png 512x512); play.exe project"), "{}", icon.summary);
        let patched = assemble_with_icon(&base, &content, &out, &icon).unwrap();
        assert_eq!(check_ico(&patched), Ok(256));
        assert_eq!(first_pixel(&out.join(PROJECT_ICON_FILE)), [200, 30, 30, 255]);
        let provenance = std::fs::read_to_string(out.join(PROVENANCE)).unwrap();
        assert!(provenance.contains("icon resource was replaced"));
    }

    #[test]
    fn no_game_icon_falls_back_to_roves_and_drops_the_sample_icon() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, out, work) = (tmp.path().join("dist"), tmp.path().join("out"), tmp.path().join("work"));
        let base = icon_base(&tmp.path().join("pkg"), true);
        dist(&content);
        let icon = resolve_project_icon(None, &content, &base.root, &work).unwrap();
        assert!(icon.window_png.is_none());
        let patched = assemble_with_icon(&base, &content, &out, &icon).unwrap();
        assert_eq!(patched, base.root.join(ROVES_FALLBACK_ICO));
        assert!(!out.join(PROJECT_ICON_FILE).exists(), "the sample game's icon must not leak");
        assert!(out.join(ROVES_FALLBACK_ICO).is_file());
    }

    #[test]
    fn an_old_base_without_the_fallback_leaves_play_exe_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, out, work) = (tmp.path().join("dist"), tmp.path().join("out"), tmp.path().join("work"));
        let base = icon_base(&tmp.path().join("pkg"), false);
        dist(&content);
        let icon = resolve_project_icon(None, &content, &base.root, &work).unwrap();
        assert!(icon.exe_ico.is_none() && icon.summary.contains("play.exe unchanged"));
        assemble(&base, &content, &out, false, &icon, &no_patch).unwrap();
        let provenance = std::fs::read_to_string(out.join(PROVENANCE)).unwrap();
        assert!(provenance.contains("unmodified"));
    }

    #[test]
    fn a_favicon_only_game_gets_its_icon_everywhere() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, out, work) = (tmp.path().join("dist"), tmp.path().join("out"), tmp.path().join("work"));
        let base = icon_base(&tmp.path().join("pkg"), true);
        dist(&content);
        let favicon = ico(&content.join("favicon.ico"), 48, [30, 30, 200, 255]);
        let icon = resolve_project_icon(None, &content, &base.root, &work).unwrap();
        let patched = assemble_with_icon(&base, &content, &out, &icon).unwrap();
        assert_eq!(patched, favicon);
        let window = image::open(out.join(PROJECT_ICON_FILE)).unwrap();
        assert_eq!((window.width(), window.height()), (48, 48));
        assert_eq!(first_pixel(&out.join(PROJECT_ICON_FILE)), [30, 30, 200, 255]);
    }

    #[test]
    fn the_release_info_icon_wins_and_must_be_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work) = (tmp.path().join("dist"), tmp.path().join("work"));
        let base = icon_base(&tmp.path().join("pkg"), true);
        dist(&content);
        png(&content.join("icon.png"), 256, [1, 1, 1, 255]);
        let own = png(&tmp.path().join("own.png"), 128, [10, 200, 10, 255]);
        let icon = resolve_project_icon(Some(&own), &content, &base.root, &work).unwrap();
        assert_eq!(icon.window_png.as_deref(), Some(own.as_path()));
        let bad = tmp.path().join("bad.png");
        std::fs::write(&bad, b"\x89PNG\r\n\x1a\n not really").unwrap();
        let error = resolve_project_icon(Some(&bad), &content, &base.root, &work).unwrap_err();
        assert!(error.starts_with("Release info icon"), "{error}");
    }

    #[test]
    fn malformed_or_out_of_bounds_game_icons_warn_and_fall_back() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work) = (tmp.path().join("dist"), tmp.path().join("work"));
        let base = icon_base(&tmp.path().join("pkg"), true);
        dist(&content);
        write(&content, "icon.png", b"<html>not an image</html>");
        write(&content, "favicon.ico", &[0, 0, 1, 0, 1, 0].iter().copied().chain([0xff; 30]).collect::<Vec<u8>>());
        let icon = resolve_project_icon(None, &content, &base.root, &work).unwrap();
        assert!(icon.summary.contains("window roves") && icon.summary.contains("play.exe roves"), "{}", icon.summary);
        assert_eq!(icon.warnings.len(), 2, "{:?}", icon.warnings);
        for (name, edge, height) in [("non-square", 256, 128), ("tiny", 8, 8), ("huge", 4096, 4096)] {
            let path = tmp.path().join(format!("{name}.png"));
            image::RgbaImage::new(edge, height).save_with_format(&path, image::ImageFormat::Png).unwrap();
            assert!(check_png(&path).is_err(), "{name}");
        }
        let good = png(&tmp.path().join("good.png"), 256, [0, 0, 0, 255]);
        let bytes = std::fs::read(&good).unwrap();
        std::fs::write(&good, &bytes[..bytes.len() / 2]).unwrap();
        assert!(check_png(&good).is_err(), "truncated");
    }

    fn dist(root: &Path) {
        write(root, "index.html", b"<p>mine</p>");
        write(root, "assets/app.js", b"mine");
    }

    #[test]
    fn disabled_output_from_an_mcp_base_has_no_marker_nor_mcp_assets() {
        let tmp = tempfile::tempdir().unwrap();
        let (base_dir, content, out) = (
            tmp.path().join("art/pkg"),
            tmp.path().join("dist"),
            tmp.path().join("out"),
        );
        fake_base(&base_dir, true);
        dist(&content);
        let base = inspect_base(&locate_base(&tmp.path().join("art")).unwrap()).unwrap();
        assert!(base.mcp_capable);
        assemble(&base, &content, &out, false, &ProjectIcon::default(), &no_patch).unwrap();
        let listed = files(&out);
        assert!(
            listed
                .iter()
                .all(|f| !f.starts_with("mcp/") && !f.starts_with("provenance/mcp-")),
            "{listed:?}"
        );
        assert!(
            listed
                .iter()
                .all(|f| !f.starts_with("provenance/visual-novel-template-"))
        );
        assert!(listed.contains(&"game/assets/app.js".to_string()));
        assert!(!listed.contains(&"game/assets/sample.js".to_string()));
        assert_eq!(
            std::fs::read(out.join("game/index.html")).unwrap(),
            b"<p>mine</p>"
        );
        let provenance = std::fs::read_to_string(out.join(PROVENANCE)).unwrap();
        assert!(!provenance.contains(MCP_PROVENANCE_HEADER));
        assert!(provenance.contains("Local MCP bridge: OFF"));
        // The regenerated checksums describe the output exactly.
        verify_checksums(&out).unwrap();
        assert!(!inspect_base(&out).unwrap().mcp_capable);
    }

    #[test]
    fn enabled_output_keeps_the_verified_mcp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let (base_dir, content, out) = (
            tmp.path().join("pkg"),
            tmp.path().join("dist"),
            tmp.path().join("out"),
        );
        fake_base(&base_dir, true);
        dist(&content);
        let base = inspect_base(&base_dir).unwrap();
        assemble(&base, &content, &out, true, &ProjectIcon::default(), &no_patch).unwrap();
        for file in MCP_REQUIRED {
            assert!(out.join(file).is_file(), "{file}");
        }
        assert!(inspect_base(&out).unwrap().mcp_capable);
    }

    #[test]
    fn enabling_mcp_on_a_plain_base_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (base_dir, content, out) = (
            tmp.path().join("pkg"),
            tmp.path().join("dist"),
            tmp.path().join("out"),
        );
        fake_base(&base_dir, false);
        dist(&content);
        let base = inspect_base(&base_dir).unwrap();
        assert!(!base.mcp_capable);
        assert!(assemble(&base, &content, &out, true, &ProjectIcon::default(), &no_patch).is_err());
        assert!(!out.exists());
        assemble(&base, &content, &out, false, &ProjectIcon::default(), &no_patch).unwrap();
        assert!(!out.join(MCP_DIR).exists());
    }

    #[test]
    fn a_marker_added_by_hand_does_not_make_a_base_mcp_capable() {
        let tmp = tempfile::tempdir().unwrap();
        let base_dir = tmp.path().join("pkg");
        fake_base(&base_dir, false);
        write(&base_dir, "mcp/enabled", b"");
        assert!(inspect_base(&base_dir).unwrap_err().contains("mcp/enabled"));
    }

    #[test]
    fn a_modified_base_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let base_dir = tmp.path().join("pkg");
        fake_base(&base_dir, true);
        write(&base_dir, "play.dll", b"patched");
        assert!(inspect_base(&base_dir).unwrap_err().contains("play.dll"));
        let other = tmp.path().join("other");
        fake_base(&other, false);
        std::fs::remove_file(other.join("libcef.dll")).unwrap();
        assert!(inspect_base(&other).is_err());
    }

    #[test]
    fn content_without_index_html_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (base_dir, content) = (tmp.path().join("pkg"), tmp.path().join("dist"));
        fake_base(&base_dir, false);
        write(&content, "main.js", b"x");
        let base = inspect_base(&base_dir).unwrap();
        assert!(assemble(&base, &content, &tmp.path().join("out"), false, &ProjectIcon::default(), &no_patch).is_err());
    }
}
