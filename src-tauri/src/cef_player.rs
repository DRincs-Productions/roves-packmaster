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
//! Every base is checked against its own `SHA256SUMS.txt` first (every file listed, every
//! listed file present with that hash, nothing unlisted), so a modified or partially extracted
//! artifact is rejected. That proves the files are the ones the CI packaged, not that the CI
//! run that produced them was green: picking an artifact from a successful run stays the
//! user's job (Packmaster has no GitHub credentials to check it).

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

const CHECKSUMS: &str = "SHA256SUMS.txt";
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
        // Provenance of the base's own sample game, which the output no longer contains.
        || rel.starts_with("provenance/visual-novel-template-")
        || (!enable_mcp && (first == MCP_DIR || rel.starts_with("provenance/mcp-")))
}

/// Copies `base` to `dest` (which must not exist) with `game\` replaced by `content_dir`,
/// applies the MCP choice, rewrites `PROVENANCE.txt` and regenerates `SHA256SUMS.txt`.
pub fn assemble(
    base: &BasePackage,
    content_dir: &Path,
    dest: &Path,
    enable_mcp: bool,
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

    let provenance = std::fs::read_to_string(root.join(PROVENANCE)).map_err(|e| e.to_string())?;
    std::fs::write(
        dest.join(PROVENANCE),
        rewrite_provenance(&provenance, enable_mcp),
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

fn rewrite_provenance(base: &str, enable_mcp: bool) -> String {
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
         \x20 the \"Game\" section above; the sample game's provenance files were removed. The\r\n\
         \x20 engine files (play.exe, play.dll, CEF runtime) are the base package's, unmodified.\r\n",
    );
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
        assemble(&base, &content, &out, false).unwrap();
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
        assemble(&base, &content, &out, true).unwrap();
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
        assert!(assemble(&base, &content, &out, true).is_err());
        assert!(!out.exists());
        assemble(&base, &content, &out, false).unwrap();
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
        assert!(assemble(&base, &content, &tmp.path().join("out"), false).is_err());
    }
}
