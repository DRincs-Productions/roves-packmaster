//! Desktop packages built on the Roves CEF players (engine v0.9.0+, the release assets
//! `roves_shell_<platform>[_steam|_mcp].zip` that `shell.rs` downloads). Each asset is a
//! complete, playable package with a sample game; Packmaster keeps the engine files untouched
//! and replaces the game:
//!
//! | | package root | executable | `game/` and window `icon.png` |
//! | --- | --- | --- | --- |
//! | Windows | folder | `play.exe` | next to `play.exe` |
//! | macOS | `roves-cef-macos-gate.app` | `Contents/MacOS/roves-cef-macos-gate` | `Contents/Resources/` |
//! | Linux | folder | `roves-cef-linux-gate` | next to the executable |
//!
//! (engine `support/cef-netrender-probe/README.md` "Packaged player", `support/cef-macos-gate/
//! README.md` "Packaged game", `support/cef-linux-gate/README.md` "Packaged game"). The zip may
//! hold the package root directly or inside a single top folder.
//!
//! **Assembly is zip to zip**: every engine entry is copied raw (same bytes, compression, Unix
//! mode and file type) into the output zip, so executables, the setuid `chrome-sandbox` mode
//! and any symlink survive whichever host Packmaster runs on; only the replaced files are
//! written anew. Installers extract that output zip again (`bundle.rs`).
//!
//! **Windows**: the base is checked against its own `SHA256SUMS.txt` (every file listed, every
//! listed file present with that hash, nothing unlisted) and the output gets a regenerated one.
//! macOS/Linux packages ship no checksums; their required files are checked instead.
//!
//! **Local MCP bridge** (every platform): it turns on only when the package contains the
//! marker `mcp/enabled` (engine `support/cef-netrender-probe/mcp/README.md`), in the folder
//! that holds `game/`: next to `play.exe`, in `Contents/Resources/` of the `.app`, next to
//! the Linux executable. With MCP off (default) the output never contains that `mcp/` folder
//! nor the MCP provenance; with MCP on, the base (the `_mcp` asset) must be a complete
//! MCP-enabled package (and, on Windows, say so in its `PROVENANCE.txt`). Packmaster never adds
//! MCP files of its own.
//!
//! **Project icon** (engine READMEs "Project icon"/"Application icon"; the same rules as the
//! engine's and roves-action's `project_icon.py`, keep them in step): the game's icon comes
//! first, the official Roves icon is the fallback. Window icon (all platforms): an `icon.png`
//! where the player reads it (the Release info icon, else `<build>/icon.png`, else the largest
//! frame of `<build>/icon.ico`/`favicon.ico`; none: the player shows the Roves logo). Windows
//! `play.exe` icon resource: an ICO generated from that PNG, else the game's ICO, else the
//! base's `roves.ico`. macOS bundle icon: an `.icns` generated from that PNG replaces
//! `Contents/Resources/roves-cef-macos-gate.icns` (the `CFBundleIconFile`), else the base's
//! Roves icon stays. Linux has no bundle icon (no `.desktop` entry in the package). The base's
//! own `icon.png` belongs to its sample game and is never copied.
//!
//! **Steam** (`_steam` assets): the Steamworks redistributable must be in the base;
//! `steam_appid.txt` is written next to the executable (where every player reads it).

use std::collections::BTreeMap;
use std::io::{BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

const CHECKSUMS: &str = "SHA256SUMS.txt";
const PROVENANCE: &str = "PROVENANCE.txt";
/// Window icon file name, in the platform's resources folder (read by the player at startup).
const PROJECT_ICON_FILE: &str = "icon.png";
/// The official Roves icon the Windows base ships, the `play.exe` fallback.
const ROVES_FALLBACK_ICO: &str = "roves.ico";
const STEAM_APP_ID_FILE: &str = "steam_appid.txt";
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;
const MIN_ICON_PX: u32 = 16;
const MAX_ICON_PX: u32 = 2048;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
/// Frames of a generated `play.exe` icon (those not larger than the source).
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const GAME_DIR: &str = "game";
const MCP_DIR: &str = "mcp";
/// Written by the engine CI's "assemble the MCP-enabled package" step, at the end of the
/// Windows `PROVENANCE.txt` (the macOS/Linux packages have none).
const MCP_PROVENANCE_HEADER: &str = "MCP-enabled development package";
/// Files of a complete MCP-enabled package, relative to `Platform::mcp_dir`.
const MCP_REQUIRED: &[&str] = &[
    "enabled",
    "server.mjs",
    "README.md",
    "node_modules/@modelcontextprotocol/server/package.json",
];
const WINDOWS_EXE: &str = "play.exe";
const MACOS_INFO_PLIST: &str = "Contents/Info.plist";
const MACOS_CEF_FRAMEWORK: &str = "Contents/Frameworks/Chromium Embedded Framework.framework";
/// Executables of the Linux package that are not the player (CEF's own helpers).
const LINUX_NOT_THE_PLAYER: &[&str] = &["chrome-sandbox", "chrome_crashpad_handler"];
/// Mode of the files Packmaster writes (game files, icons, `steam_appid.txt`).
const FILE_MODE: u32 = 0o644;
const DIR_MODE: u32 = 0o755;
const EXE_MODE: u32 = 0o755;

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

    /// `windows` | `macos` | `linux`, as in the engine's release asset names.
    pub fn id(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
        }
    }

    /// File that identifies the package root inside the zip (as roves-action's
    /// `cef/assemble.sh` locates it: the `.app` on macOS, the folder of `libcef.so` on Linux).
    fn root_marker(self) -> &'static str {
        match self {
            Self::Windows => WINDOWS_EXE,
            Self::Macos => MACOS_INFO_PLIST,
            Self::Linux => "libcef.so",
        }
    }

    /// Folder of the executable: `steam_appid.txt` and the Steamworks redistributable.
    fn exe_dir(self) -> &'static str {
        match self {
            Self::Macos => "Contents/MacOS",
            _ => "",
        }
    }

    /// Folder holding `game/` and the window `icon.png`.
    fn resources_dir(self) -> &'static str {
        match self {
            Self::Macos => "Contents/Resources",
            _ => "",
        }
    }

    fn required(self) -> &'static [&'static str] {
        match self {
            Self::Windows => &[WINDOWS_EXE, "play.dll", "play.exe.manifest", "libcef.dll", PROVENANCE, CHECKSUMS],
            // Plus the CFBundleExecutable, the CFBundleIconFile and the CEF framework.
            Self::Macos => &[MACOS_INFO_PLIST, "Contents/Resources/game/index.html"],
            Self::Linux => &["libcef.so", "chrome-sandbox", "icudtl.dat", "resources.pak", "game/index.html"],
        }
    }

    /// The local MCP bridge's folder (`mcp`, next to `game/`).
    fn mcp_dir(self) -> String {
        join(self.resources_dir(), MCP_DIR)
    }

    fn steam_library(self) -> &'static str {
        match self {
            Self::Windows => "steam_api64.dll",
            Self::Macos => "Contents/MacOS/libsteam_api.dylib",
            Self::Linux => "libsteam_api.so",
        }
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// The archive prefix (`""` or `"<top folder>/"`) of the only package root in `names`.
fn locate_root<'a>(names: impl Iterator<Item = &'a str>, platform: Platform) -> Result<String, String> {
    let marker = platform.root_marker();
    let mut roots = std::collections::BTreeSet::new();
    for name in names {
        let prefix = if name == marker {
            ""
        } else if let Some(prefix) = name.strip_suffix(marker).filter(|p| p.ends_with('/')) {
            prefix
        } else {
            continue;
        };
        if prefix.matches('/').count() <= 1 {
            roots.insert(prefix.to_string());
        }
    }
    let mut roots = roots.into_iter();
    match (roots.next(), roots.next()) {
        (Some(root), None) => Ok(root),
        (None, _) => Err(format!(
            "not a Roves {} player package (no {marker} at its root or in a single top folder)",
            platform.id()
        )),
        (Some(_), Some(_)) => Err(format!("contains more than one Roves {} player package", platform.id())),
    }
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    index: usize,
    is_dir: bool,
    unix_mode: Option<u32>,
}

/// An engine player package (release asset zip), checked and ready to assemble from.
pub struct Base {
    archive: zip::ZipArchive<std::fs::File>,
    pub platform: Platform,
    /// Package-relative path (forward slashes, no trailing slash) -> archive entry, for every
    /// entry under the package root.
    entries: BTreeMap<String, Entry>,
    /// Verified `SHA256SUMS.txt` (path -> lowercase hex), when the package ships one.
    checksums: Option<BTreeMap<String, String>>,
    /// Complete MCP-enabled package (every MCP file in `Platform::mcp_dir`, plus the MCP
    /// section of `PROVENANCE.txt` when the package has one).
    pub mcp_capable: bool,
    /// The player executable, relative to the package root: `play.exe`, the macOS
    /// `CFBundleExecutable` under `Contents/MacOS/`, or the Linux package's only executable
    /// that is not a CEF helper.
    pub executable: String,
    /// macOS: the bundle icon named by `CFBundleIconFile`, under `Contents/Resources/`.
    bundle_icns: Option<String>,
}

impl Base {
    /// Opens and checks `zip_path`: the platform's required files (plus the Steamworks
    /// redistributable when `steam`), `SHA256SUMS.txt` when present (always on Windows), and
    /// the MCP capability.
    pub fn open(zip_path: &Path, platform: Platform, steam: bool) -> Result<Self, String> {
        let file = std::fs::File::open(zip_path).map_err(|e| format!("{}: {e}", zip_path.display()))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", zip_path.display()))?;
        let mut names = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?.replace('\\', "/");
            let is_dir = entry.is_dir() || name.ends_with('/');
            names.push((name, is_dir, entry.unix_mode()));
        }
        let prefix = locate_root(names.iter().map(|(name, _, _)| name.as_str()), platform)
            .map_err(|e| format!("{}: {e}", zip_path.display()))?;
        let mut entries = BTreeMap::new();
        for (index, (name, is_dir, unix_mode)) in names.iter().enumerate() {
            let Some(rel) = name.strip_prefix(&prefix) else { continue };
            let rel = rel.trim_end_matches('/');
            if rel.is_empty() {
                continue;
            }
            if Path::new(rel).components().any(|c| !matches!(c, Component::Normal(_))) {
                return Err(format!("the {} player package has an unsafe path: {name:?}", platform.id()));
            }
            entries.insert(
                rel.to_string(),
                Entry {
                    index,
                    is_dir: *is_dir,
                    unix_mode: *unix_mode,
                },
            );
        }
        let mut base = Self {
            archive,
            platform,
            entries,
            checksums: None,
            mcp_capable: false,
            executable: WINDOWS_EXE.to_string(),
            bundle_icns: None,
        };
        let lacks = |rel: &str| format!("the {} player package lacks {rel}", platform.id());
        for required in platform.required() {
            if !base.is_file(required) {
                return Err(lacks(required));
            }
        }
        match platform {
            Platform::Windows => {},
            Platform::Macos => {
                let (executable, icns) = macos_bundle_names(&base.read(MACOS_INFO_PLIST)?)?;
                for required in [&executable, &icns] {
                    if !base.is_file(required) {
                        return Err(lacks(required));
                    }
                }
                let framework = format!("{MACOS_CEF_FRAMEWORK}/");
                if !base.entries.keys().any(|rel| rel.starts_with(&framework)) {
                    return Err(lacks(MACOS_CEF_FRAMEWORK));
                }
                base.executable = executable;
                base.bundle_icns = Some(icns);
            },
            Platform::Linux => base.executable = base.linux_executable()?,
        }
        let steam_library = platform.steam_library();
        match (steam, base.is_file(steam_library)) {
            (true, false) => return Err(lacks(steam_library)),
            (false, true) => {
                return Err(format!(
                    "the standard {} player package unexpectedly contains {steam_library}",
                    platform.id()
                ));
            },
            _ => {},
        }
        if base.is_file(CHECKSUMS) {
            base.checksums = Some(base.verify_checksums()?);
        }
        let mcp_dir = platform.mcp_dir();
        base.mcp_capable = MCP_REQUIRED.iter().all(|file| base.is_file(&join(&mcp_dir, file)))
            && (!base.is_file(PROVENANCE)
                || String::from_utf8_lossy(&base.read(PROVENANCE)?)
                    .lines()
                    .any(|line| line.starts_with(MCP_PROVENANCE_HEADER)));
        Ok(base)
    }

    /// The only root-level file with an execute bit that is neither a CEF helper nor a
    /// shared library (the release zip keeps Unix modes, made with Info-ZIP).
    fn linux_executable(&self) -> Result<String, String> {
        let candidates: Vec<&String> = self
            .entries
            .iter()
            .filter(|(rel, entry)| {
                !entry.is_dir
                    && !rel.contains('/')
                    && entry.unix_mode.is_some_and(|mode| mode & 0o111 != 0)
                    && !LINUX_NOT_THE_PLAYER.contains(&rel.as_str())
                    && !rel.contains(".so")
            })
            .map(|(rel, _)| rel)
            .collect();
        match candidates.as_slice() {
            [only] => Ok(only.to_string()),
            [] => Err("the linux player package has no executable (were its Unix modes lost?)".to_string()),
            more => Err(format!("the linux player package has more than one candidate executable: {more:?}")),
        }
    }

    fn is_file(&self, rel: &str) -> bool {
        self.entries.get(rel).is_some_and(|entry| !entry.is_dir)
    }

    fn read(&mut self, rel: &str) -> Result<Vec<u8>, String> {
        let entry = *self
            .entries
            .get(rel)
            .ok_or_else(|| format!("the player package lacks {rel}"))?;
        let mut file = self.archive.by_index(entry.index).map_err(|e| format!("{rel}: {e}"))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| format!("{rel}: {e}"))?;
        Ok(bytes)
    }

    fn extract(&mut self, rel: &str, dest: &Path) -> Result<(), String> {
        let bytes = self.read(rel)?;
        std::fs::write(dest, bytes).map_err(|e| format!("{}: {e}", dest.display()))
    }

    /// The Windows base's `roves.ico`, extracted into `work_dir` (`None` elsewhere or for
    /// bases without it).
    pub fn fallback_ico(&mut self, work_dir: &Path) -> Result<Option<PathBuf>, String> {
        if self.platform != Platform::Windows || !self.is_file(ROVES_FALLBACK_ICO) {
            return Ok(None);
        }
        std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
        let dest = work_dir.join(ROVES_FALLBACK_ICO);
        self.extract(ROVES_FALLBACK_ICO, &dest)?;
        Ok(Some(dest))
    }

    fn verify_checksums(&mut self) -> Result<BTreeMap<String, String>, String> {
        let text = String::from_utf8(self.read(CHECKSUMS)?).map_err(|e| format!("{CHECKSUMS}: {e}"))?;
        let mut listed = BTreeMap::new();
        for line in text.trim_start_matches('\u{feff}').lines().filter(|l| !l.trim().is_empty()) {
            let (hash, path) = line
                .split_once("  ")
                .ok_or_else(|| format!("malformed {CHECKSUMS} line: {line:?}"))?;
            listed.insert(path.trim_end().to_string(), hash.to_ascii_lowercase());
        }
        let files: Vec<(String, Entry)> = self
            .entries
            .iter()
            .filter(|(rel, entry)| !entry.is_dir && rel.as_str() != CHECKSUMS)
            .map(|(rel, entry)| (rel.clone(), *entry))
            .collect();
        if let Some((extra, _)) = files.iter().find(|(rel, _)| !listed.contains_key(rel)) {
            return Err(format!("the player package contains {extra}, which its {CHECKSUMS} does not list"));
        }
        if let Some(missing) = listed.keys().find(|rel| !self.is_file(rel)) {
            return Err(format!("the player package lacks {missing}, listed in its {CHECKSUMS}"));
        }
        for (rel, entry) in files {
            let mut file = self.archive.by_index(entry.index).map_err(|e| format!("{rel}: {e}"))?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut hasher).map_err(|e| format!("{rel}: {e}"))?;
            if hex(&hasher.finalize()) != listed[&rel] {
                return Err(format!("{rel} does not match the player package's {CHECKSUMS}"));
            }
        }
        Ok(listed)
    }
}

/// Replaces the main icon group of the Windows executable `exe` (in place) with the frames of
/// `ico`, like rcedit's `--set-icon` but in-process with the `editpe` crate, so it works on
/// every host. The previous main group and the icons only it used are removed (the sample
/// game's icon must not survive); every other resource (version info, manifest, other icon
/// groups) and the rest of the image are kept.
pub fn set_exe_icon(exe: &Path, ico: &Path) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("setting the icon of {}: {e}", exe.display());
    let ico = std::fs::read(ico).map_err(|e| format!("{}: {e}", ico.display()))?;
    let mut image = editpe::Image::parse_file(exe).map_err(|e| fail(&e))?;
    let mut resources = image.resource_directory().cloned().unwrap_or_default();
    resources.remove_main_icon().map_err(|e| fail(&e))?;
    resources.set_main_icon(ico).map_err(|e| fail(&e))?;
    image.set_resource_directory(resources).map_err(|e| fail(&e))?;
    image.write_file(exe).map_err(|e| fail(&e))
}

/// `(Contents/MacOS/<CFBundleExecutable>, Contents/Resources/<CFBundleIconFile>.icns)`, read
/// from `Info.plist` like roves-action's `cef/assemble.sh` does with PlistBuddy.
fn macos_bundle_names(info_plist: &[u8]) -> Result<(String, String), String> {
    let plist = plist::Value::from_reader(std::io::Cursor::new(info_plist)).map_err(|e| format!("{MACOS_INFO_PLIST}: {e}"))?;
    let key = |name: &str| {
        plist
            .as_dictionary()
            .and_then(|dict| dict.get(name))
            .and_then(plist::Value::as_string)
            .filter(|value| !value.is_empty() && !value.contains('/'))
            .map(str::to_string)
            .ok_or_else(|| format!("{MACOS_INFO_PLIST} has no usable {name}"))
    };
    let executable = key("CFBundleExecutable")?;
    let icon = key("CFBundleIconFile")?;
    Ok((
        format!("Contents/MacOS/{executable}"),
        format!("Contents/Resources/{}.icns", icon.trim_end_matches(".icns")),
    ))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Resolved project icon (see the module documentation).
#[derive(Debug, Default)]
pub struct ProjectIcon {
    /// Written as the window `icon.png`; `None`: the player shows the Roves logo.
    pub window_png: Option<PathBuf>,
    /// Windows: patched into `play.exe`; `None`: `play.exe` keeps the base's icon.
    pub exe_ico: Option<PathBuf>,
    /// macOS: replaces the bundle `.icns`; `None`: the base's Roves icon stays.
    pub app_icns: Option<PathBuf>,
    /// `window <project|roves> (...)`, plus the platform's executable/bundle icon.
    pub summary: String,
    /// Invalid icon files found in the build output and skipped, or icons not applied.
    pub warnings: Vec<String>,
}

fn read_icon_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("not a file".to_string());
    }
    if metadata.len() > MAX_ICON_BYTES {
        return Err(format!("{} bytes exceeds the {MAX_ICON_BYTES}-byte limit", metadata.len()));
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

fn check_edge(width: u32, height: u32) -> Result<(), String> {
    if width != height {
        return Err(format!("{width}x{height} is not square"));
    }
    if !(MIN_ICON_PX..=MAX_ICON_PX).contains(&width) {
        return Err(format!("{width}x{height} is outside {MIN_ICON_PX}..={MAX_ICON_PX} px"));
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
    let image = image::load_from_memory_with_format(&data, image::ImageFormat::Png).map_err(|e| format!("decode: {e}"))?;
    if (image.width(), image.height()) != (width, height) {
        return Err(format!("decoded as {}x{}", image.width(), image.height()));
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
            image::load_from_memory_with_format(&single, image::ImageFormat::Ico).map_err(|e| format!("decode frame {i}: {e}"))
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
        return Err(format!("largest frame {width}x{height} is not a square of at least {MIN_ICON_PX} px"));
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

/// Resolves the project icon for `platform`, writing generated files into `work_dir`
/// (created if needed). `explicit_png` is Packmaster's Release info icon; `fallback_ico` the
/// Windows base's `roves.ico` (`Base::fallback_ico`).
pub fn resolve_project_icon(
    platform: Platform,
    explicit_png: Option<&Path>,
    content_dir: &Path,
    fallback_ico: Option<&Path>,
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
        (Some((path.clone(), label.clone(), *edge)), format!("project ({label} {edge}x{edge})"))
    } else if let Some((path, label, edge)) = &ico {
        let data = read_icon_file(path)?;
        let frame = largest_frame(ico_frames(&data)?).ok_or("no frame")?;
        let out = work_dir.join(PROJECT_ICON_FILE);
        frame
            .to_rgba8()
            .save_with_format(&out, image::ImageFormat::Png)
            .map_err(|e| format!("writing {}: {e}", out.display()))?;
        let label = format!("largest frame of {label}");
        (Some((out, label.clone(), check_png(&work_dir.join(PROJECT_ICON_FILE))?)), format!("project ({label} {edge}x{edge})"))
    } else {
        (None, "roves (no project icon: embedded Roves logo)".to_string())
    };

    let mut icon = ProjectIcon {
        window_png: window_png.as_ref().map(|(path, _, _)| path.clone()),
        ..ProjectIcon::default()
    };
    let second = match platform {
        Platform::Windows => {
            let (exe_ico, exe) = if let Some((path, label, edge)) = &png {
                let out = work_dir.join("play.ico");
                let sizes: Vec<u32> = ICO_SIZES.into_iter().filter(|&size| size <= *edge).collect();
                crate::icon::generate_ico_with_sizes(path, &out, &sizes)?;
                check_ico(&out)?;
                (Some(out), format!("project (ICO generated from {label})"))
            } else if let Some((path, label, _)) = &ico {
                (Some(path.clone()), format!("project ({label})"))
            } else if let Some(fallback) = fallback_ico {
                check_ico(fallback).map_err(|e| format!("the base's {ROVES_FALLBACK_ICO}: {e}"))?;
                (Some(fallback.to_path_buf()), "roves (Roves fallback icon)".to_string())
            } else {
                (None, "unchanged (no project icon and no Roves fallback icon in the base)".to_string())
            };
            icon.exe_ico = exe_ico;
            format!("; play.exe {exe}")
        },
        Platform::Macos => {
            if let Some((path, label, _)) = &window_png {
                let out = work_dir.join("app.icns");
                crate::icon::generate_icns(path, &out)?;
                icon.app_icns = Some(out);
                format!("; app icon project (ICNS generated from {label})")
            } else {
                "; app icon roves (the base's Roves icon)".to_string()
            }
        },
        Platform::Linux => String::new(),
    };
    icon.summary = format!("window {window}{second}");
    icon.warnings = warnings;
    Ok(icon)
}

pub struct AssembleOptions<'a> {
    /// The developer's build output (must contain `index.html`).
    pub content_dir: &'a Path,
    /// Top-level folder of the output zip: `<name>` (Windows/Linux) or `<name>.app` (macOS).
    pub top_folder: &'a str,
    /// Keep the base's local MCP bridge (the `_mcp` asset's `mcp/` folder).
    pub enable_mcp: bool,
    /// Written to `steam_appid.txt` next to the executable.
    pub steam_app_id: Option<&'a str>,
    pub icon: &'a ProjectIcon,
    /// Scratch directory (the patched `play.exe`).
    pub work_dir: &'a Path,
}

enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// Writes `out_zip`: the base's entries (raw copies) with the game replaced by
/// `opts.content_dir`, the MCP choice, the resolved project icon (`patch_exe(play.exe, ico)`
/// patches the Windows executable's icon resource, `set_exe_icon` in production), `steam_appid.txt`,
/// a rewritten `PROVENANCE.txt` and a regenerated `SHA256SUMS.txt` when the base has them.
pub fn assemble(
    base: &mut Base,
    opts: &AssembleOptions,
    out_zip: &Path,
    patch_exe: &dyn Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let platform = base.platform;
    if opts.enable_mcp && !base.mcp_capable {
        return Err(format!(
            "the local MCP bridge is enabled, but the {} player package is not a complete MCP-enabled package",
            platform.id()
        ));
    }
    if !opts.content_dir.join("index.html").is_file() {
        return Err(format!("{} has no index.html at its root", opts.content_dir.display()));
    }
    if out_zip.exists() {
        std::fs::remove_file(out_zip).map_err(|e| format!("{}: {e}", out_zip.display()))?;
    }
    let result = write_package(base, opts, out_zip, patch_exe);
    if result.is_err() {
        std::fs::remove_file(out_zip).ok();
    }
    result
}

fn write_package(
    base: &mut Base,
    opts: &AssembleOptions,
    out_zip: &Path,
    patch_exe: &dyn Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let platform = base.platform;
    let resources = platform.resources_dir();
    let game_dir = join(resources, GAME_DIR);
    let window_icon = join(resources, PROJECT_ICON_FILE);
    let steam_app_id_file = join(platform.exe_dir(), STEAM_APP_ID_FILE);
    let mcp_dir = platform.mcp_dir();
    let in_mcp_dir = |rel: &str| rel == mcp_dir || rel.starts_with(&format!("{mcp_dir}/"));
    let mcp_marker = join(&mcp_dir, MCP_REQUIRED[0]);

    // Files Packmaster writes, (package-relative path, source).
    let mut added: Vec<(String, Source)> = Vec::new();
    let mut added_dirs = vec![game_dir.clone()];
    for entry in walkdir::WalkDir::new(opts.content_dir).min_depth(1).sort_by_file_name() {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(opts.content_dir).map_err(|e| e.to_string())?;
        let rel = format!("{game_dir}/{}", rel.to_string_lossy().replace('\\', "/"));
        if entry.file_type().is_dir() {
            added_dirs.push(rel);
        } else {
            added.push((rel, Source::File(entry.path().to_path_buf())));
        }
    }
    if let Some(png) = &opts.icon.window_png {
        added.push((window_icon.clone(), Source::File(png.clone())));
    }
    if let (Some(icns), Some(bundle_icns)) = (&opts.icon.app_icns, &base.bundle_icns) {
        added.push((bundle_icns.clone(), Source::File(icns.clone())));
    }
    if let Some(ico) = &opts.icon.exe_ico {
        std::fs::create_dir_all(opts.work_dir).map_err(|e| e.to_string())?;
        let exe = opts.work_dir.join(WINDOWS_EXE);
        base.extract(WINDOWS_EXE, &exe)?;
        patch_exe(&exe, ico)?;
        added.push((WINDOWS_EXE.to_string(), Source::File(exe)));
    }
    if let Some(app_id) = opts.steam_app_id {
        added.push((steam_app_id_file.clone(), Source::Bytes(app_id.as_bytes().to_vec())));
    }
    if base.is_file(PROVENANCE) {
        let provenance = String::from_utf8_lossy(&base.read(PROVENANCE)?).into_owned();
        added.push((
            PROVENANCE.to_string(),
            Source::Bytes(rewrite_provenance(&provenance, platform, &base.executable, opts.enable_mcp, opts.icon).into_bytes()),
        ));
    }

    let replaced: std::collections::BTreeSet<&str> = added.iter().map(|(rel, _)| rel.as_str()).collect();
    let excluded = |rel: &str| {
        rel == game_dir
            || rel.starts_with(&format!("{game_dir}/"))
            || rel == window_icon
            || rel == steam_app_id_file
            || rel == CHECKSUMS
            || replaced.contains(rel)
            // Provenance of the base's own sample game, which the output no longer contains.
            || rel.starts_with("provenance/visual-novel-template-")
            || (!opts.enable_mcp && (in_mcp_dir(rel) || rel.starts_with("provenance/mcp-")))
    };
    let kept: Vec<(String, Entry)> = base
        .entries
        .iter()
        .filter(|(rel, _)| !excluded(rel))
        .map(|(rel, entry)| (rel.clone(), *entry))
        .collect();

    let top = opts.top_folder;
    let file = std::fs::File::create(out_zip).map_err(|e| format!("{}: {e}", out_zip.display()))?;
    let mut writer = zip::ZipWriter::new(BufWriter::new(file));
    let mut hashes = BTreeMap::new();
    for (rel, entry) in &kept {
        let raw = base.archive.by_index_raw(entry.index).map_err(|e| format!("{rel}: {e}"))?;
        let name = if entry.is_dir { format!("{top}/{rel}/") } else { format!("{top}/{rel}") };
        writer.raw_copy_file_rename(raw, name).map_err(|e| format!("{rel}: {e}"))?;
        if let (false, Some(sums)) = (entry.is_dir, &base.checksums) {
            hashes.insert(rel.clone(), sums[rel].clone());
        }
    }
    // Unix entries on every host, like the engine's own zips (a DOS entry would hide the modes).
    let dir_options = SimpleFileOptions::default()
        .system(zip::System::Unix)
        .unix_permissions(DIR_MODE);
    for dir in &added_dirs {
        writer
            .add_directory(format!("{top}/{dir}/"), dir_options)
            .map_err(|e| format!("{dir}: {e}"))?;
    }
    let file_options = SimpleFileOptions::default()
        .system(zip::System::Unix)
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true)
        .unix_permissions(FILE_MODE);
    for (rel, source) in &added {
        let options = if *rel == base.executable { file_options.unix_permissions(EXE_MODE) } else { file_options };
        writer
            .start_file(format!("{top}/{rel}"), options)
            .map_err(|e| format!("{rel}: {e}"))?;
        let mut hasher = Sha256::new();
        match source {
            Source::Bytes(bytes) => {
                hasher.update(bytes);
                writer.write_all(bytes).map_err(|e| format!("{rel}: {e}"))?;
            },
            Source::File(path) => {
                let mut reader = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
                let mut buffer = vec![0u8; 1 << 16];
                loop {
                    let read = reader.read(&mut buffer).map_err(|e| format!("{}: {e}", path.display()))?;
                    if read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..read]);
                    writer.write_all(&buffer[..read]).map_err(|e| format!("{rel}: {e}"))?;
                }
            },
        }
        hashes.insert(rel.clone(), hex(&hasher.finalize()));
    }
    if base.checksums.is_some() {
        writer
            .start_file(format!("{top}/{CHECKSUMS}"), file_options)
            .map_err(|e| e.to_string())?;
        for (rel, hash) in &hashes {
            write!(writer, "{hash}  {rel}\r\n").map_err(|e| e.to_string())?;
        }
    }

    let has_marker = hashes.contains_key(&mcp_marker) || kept.iter().any(|(rel, _)| *rel == mcp_marker);
    let has_mcp_dir = kept.iter().any(|(rel, _)| in_mcp_dir(rel));
    if opts.enable_mcp != has_marker || (!opts.enable_mcp && has_mcp_dir) {
        return Err("internal error: the MCP state of the assembled package does not match the setting".to_string());
    }
    writer
        .finish()
        .map_err(|e| format!("writing {}: {e}", out_zip.display()))?
        .flush()
        .map_err(|e| format!("writing {}: {e}", out_zip.display()))
}

fn rewrite_provenance(base: &str, platform: Platform, exe: &str, enable_mcp: bool, icon: &ProjectIcon) -> String {
    let mut lines: Vec<&str> = base.trim_start_matches('\u{feff}').lines().collect();
    if !enable_mcp {
        if let Some(at) = lines.iter().position(|line| line.starts_with(MCP_PROVENANCE_HEADER)) {
            lines.truncate(at);
        }
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    let mut out = lines.join("\r\n");
    out.push_str(
        "\r\n\r\nGame replaced by Roves Packmaster\r\n\
         \x20 the game folder contains the developer's own build output, not the sample game\r\n\
         \x20 described above; the sample game's provenance files were removed.\r\n",
    );
    out.push_str(&if icon.exe_ico.is_some() {
        format!("  engine files are the base package's; only {exe}'s icon resource was replaced\r\n  (editpe).\r\n")
    } else {
        "  engine files are the base package's, unmodified.\r\n".to_string()
    });
    if !icon.summary.is_empty() {
        out.push_str(&format!("  icons: {}\r\n", icon.summary));
    }
    if platform == Platform::Windows {
        out.push_str(if enable_mcp {
            "  Local MCP bridge: ON (mcp\\enabled kept). Development and playtests only: any program\r\n\
             \x20 running as your Windows user can read and control the running game. Never ship to players.\r\n"
        } else {
            "  Local MCP bridge: OFF (no mcp\\ folder, no activation marker).\r\n"
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXEC: u32 = 0o755;
    const LINUX_EXE: &str = "roves-cef-linux-gate";
    const MACOS_EXE: &str = "Contents/MacOS/roves-cef-macos-gate";
    const MACOS_ICNS: &str = "Contents/Resources/roves-cef-macos-gate.icns";
    /// The keys cef's bundler writes that the packaging reads.
    const MACOS_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>roves-cef-macos-gate</string>
<key>CFBundleIconFile</key><string>roves-cef-macos-gate</string>
</dict></plist>"#;

    /// `(package-relative path, bytes, unix mode)`.
    type Files = BTreeMap<String, (Vec<u8>, u32)>;

    fn put(files: &mut Files, rel: &str, bytes: &[u8], mode: u32) {
        files.insert(rel.to_string(), (bytes.to_vec(), mode));
    }

    fn with_checksums(files: &mut Files) {
        let mut sums = String::new();
        for (rel, (bytes, _)) in files.iter() {
            sums.push_str(&format!("{}  {rel}\r\n", hex(&Sha256::digest(bytes))));
        }
        put(files, CHECKSUMS, sums.as_bytes(), FILE_MODE);
    }

    /// Writes `files` (and directory entries for their parents) under `top` (`""`: zip root).
    fn write_zip(path: &Path, top: &str, files: &Files) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let prefix = if top.is_empty() { String::new() } else { format!("{top}/") };
        let mut dirs = std::collections::BTreeSet::new();
        for rel in files.keys() {
            let mut parent = Path::new(rel).parent();
            while let Some(dir) = parent.filter(|d| !d.as_os_str().is_empty()) {
                dirs.insert(dir.to_string_lossy().replace('\\', "/"));
                parent = dir.parent();
            }
        }
        if !prefix.is_empty() {
            writer.add_directory(prefix.clone(), SimpleFileOptions::default()).unwrap();
        }
        for dir in dirs {
            writer.add_directory(format!("{prefix}{dir}/"), SimpleFileOptions::default()).unwrap();
        }
        for (rel, (bytes, mode)) in files {
            let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            // unix_permissions() keeps only 0o777; the external attributes carry setuid too.
            let options = options
                .system(zip::System::Unix)
                .external_attributes((0o100000 | mode) << 16);
            writer.start_file(format!("{prefix}{rel}"), options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path.to_path_buf()
    }

    /// The output zip: `(name -> (bytes, unix mode))`, directories with a trailing slash.
    fn read_zip(path: &Path) -> BTreeMap<String, (Vec<u8>, Option<u32>)> {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .map(|i| {
                let mut file = archive.by_index(i).unwrap();
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                // unix_mode() clears setuid/setgid/sticky; the raw attributes keep them.
                let mode = (file.external_attributes() >> 16 != 0).then(|| file.external_attributes() >> 16);
                (file.name().unwrap().to_string(), (bytes, mode))
            })
            .collect()
    }

    fn windows_files(mcp: bool) -> Files {
        let mut files = Files::new();
        for file in ["play.exe", "play.dll", "play.exe.manifest", "libcef.dll", "locales/en-US.pak"] {
            put(&mut files, file, file.as_bytes(), FILE_MODE);
        }
        put(&mut files, "game/index.html", b"<p>sample</p>", FILE_MODE);
        put(&mut files, "game/assets/sample.js", b"sample", FILE_MODE);
        put(&mut files, "provenance/Cargo.lock", b"lock", FILE_MODE);
        put(&mut files, "provenance/visual-novel-template-npm-ls.txt", b"template", FILE_MODE);
        let mut provenance = String::from("Engine\r\n  commit abc\r\n\r\nGame (folder game\\)\r\n  template\r\n");
        if mcp {
            for file in MCP_REQUIRED {
                put(&mut files, &format!("mcp/{file}"), b"mcp", FILE_MODE);
            }
            put(&mut files, "provenance/mcp-npm-ls.txt", b"mcp deps", FILE_MODE);
            // Hidden files are part of the package too (listed in SHA256SUMS.txt).
            put(&mut files, "mcp/node_modules/.package-lock.json", b"{}", FILE_MODE);
            provenance.push_str(
                "\r\nMCP-enabled development package (enable-mcp=true; never ship to players)\r\n  mcp\\enabled\r\n",
            );
        }
        put(&mut files, PROVENANCE, provenance.as_bytes(), FILE_MODE);
        files
    }

    fn windows_base(dir: &Path, mcp: bool) -> PathBuf {
        let mut files = windows_files(mcp);
        with_checksums(&mut files);
        write_zip(&dir.join("windows.zip"), "roves", &files)
    }

    /// The engine's MCP-enabled package content under `mcp_dir` (hidden files included).
    fn put_mcp(files: &mut Files, mcp_dir: &str) {
        for file in MCP_REQUIRED {
            put(files, &format!("{mcp_dir}/{file}"), b"mcp", FILE_MODE);
        }
        put(files, &format!("{mcp_dir}/package.json"), b"{}", FILE_MODE);
        put(files, &format!("{mcp_dir}/node_modules/.package-lock.json"), b"{}", FILE_MODE);
    }

    fn linux_base(dir: &Path, steam: bool) -> PathBuf {
        linux_base_with(dir, steam, false)
    }

    fn linux_base_with(dir: &Path, steam: bool, mcp: bool) -> PathBuf {
        let mut files = Files::new();
        put(&mut files, LINUX_EXE, b"elf", EXEC);
        put(&mut files, "libcef.so", b"cef", EXEC);
        put(&mut files, "chrome-sandbox", b"sandbox", 0o4755);
        put(&mut files, "icudtl.dat", b"icu", FILE_MODE);
        put(&mut files, "resources.pak", b"pak", FILE_MODE);
        put(&mut files, "game/index.html", b"<p>sample</p>", FILE_MODE);
        if steam {
            put(&mut files, "libsteam_api.so", b"steam", EXEC);
        }
        if mcp {
            put_mcp(&mut files, "mcp");
        }
        write_zip(&dir.join("linux.zip"), "roves-cef-linux", &files)
    }

    fn macos_base(dir: &Path) -> PathBuf {
        macos_base_with(dir, false)
    }

    fn macos_base_with(dir: &Path, mcp: bool) -> PathBuf {
        let mut files = Files::new();
        put(&mut files, MACOS_INFO_PLIST, MACOS_PLIST.as_bytes(), FILE_MODE);
        put(&mut files, MACOS_EXE, b"macho", EXEC);
        put(&mut files, MACOS_ICNS, b"roves icns", FILE_MODE);
        put(&mut files, &format!("{MACOS_CEF_FRAMEWORK}/Chromium Embedded Framework"), b"cef", EXEC);
        put(&mut files, "Contents/Frameworks/roves-cef-macos-gate Helper.app/Contents/Info.plist", b"<plist/>", FILE_MODE);
        put(&mut files, "Contents/Resources/game/index.html", b"<p>sample</p>", FILE_MODE);
        put(&mut files, "Contents/Resources/icon.png", b"sample icon", FILE_MODE);
        if mcp {
            put_mcp(&mut files, "Contents/Resources/mcp");
        }
        write_zip(&dir.join("macos.zip"), "roves-cef-macos-gate.app", &files)
    }

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn dist(root: &Path) -> PathBuf {
        write(root, "index.html", b"<p>mine</p>");
        write(root, "assets/app.js", b"mine");
        root.to_path_buf()
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

    fn no_patch(_: &Path, _: &Path) -> Result<(), String> {
        panic!("play.exe must not be patched without a resolved icon")
    }

    fn options<'a>(content: &'a Path, work: &'a Path, icon: &'a ProjectIcon) -> AssembleOptions<'a> {
        AssembleOptions {
            content_dir: content,
            top_folder: "my game",
            enable_mcp: false,
            steam_app_id: None,
            icon,
            work_dir: work,
        }
    }

    fn names(zip: &BTreeMap<String, (Vec<u8>, Option<u32>)>) -> Vec<&str> {
        zip.keys().map(String::as_str).filter(|n| !n.ends_with('/')).collect()
    }

    #[test]
    fn the_package_root_is_found_at_the_root_or_in_one_top_folder() {
        use Platform::*;
        assert_eq!(locate_root(["play.exe", "game/index.html"].into_iter(), Windows).unwrap(), "");
        assert_eq!(locate_root(["pkg/", "pkg/play.exe"].into_iter(), Windows).unwrap(), "pkg/");
        assert!(locate_root(["a/b/play.exe"].into_iter(), Windows).is_err());
        assert!(locate_root(["a/play.exe", "b/play.exe"].into_iter(), Windows)
            .unwrap_err()
            .contains("more than one"));
        // The CEF helper apps' Info.plist files are deeper and never mistaken for the root.
        let mac = ["X.app/Contents/Info.plist", "X.app/Contents/Frameworks/H.app/Contents/Info.plist"];
        assert_eq!(locate_root(mac.into_iter(), Macos).unwrap(), "X.app/");
        assert!(locate_root(["play.exe"].into_iter(), Linux).is_err());
    }

    #[test]
    fn windows_game_is_replaced_and_checksums_regenerated() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work, out) = (dist(&tmp.path().join("dist")), tmp.path().join("work"), tmp.path().join("out.zip"));
        let mut base = Base::open(&windows_base(tmp.path(), true), Platform::Windows, false).unwrap();
        assert!(base.mcp_capable);
        let icon = ProjectIcon::default();
        assemble(&mut base, &options(&content, &work, &icon), &out, &no_patch).unwrap();
        let zip = read_zip(&out);
        let listed = names(&zip);
        assert!(listed.contains(&"my game/game/assets/app.js"));
        assert!(!listed.iter().any(|f| f.contains("sample.js") || f.contains("visual-novel-template")));
        assert!(!listed.iter().any(|f| f.starts_with("my game/mcp/") || f.contains("provenance/mcp-")), "{listed:?}");
        assert_eq!(zip["my game/game/index.html"].0, b"<p>mine</p>");
        let provenance = String::from_utf8(zip["my game/PROVENANCE.txt"].0.clone()).unwrap();
        assert!(!provenance.contains(MCP_PROVENANCE_HEADER) && provenance.contains("Local MCP bridge: OFF"));
        // The output is itself a valid, checksum-verified Windows package without MCP.
        let again = Base::open(&out, Platform::Windows, false).unwrap();
        assert!(!again.mcp_capable);
    }

    #[test]
    fn windows_mcp_output_keeps_the_verified_mcp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work, out) = (dist(&tmp.path().join("dist")), tmp.path().join("work"), tmp.path().join("out.zip"));
        let mut base = Base::open(&windows_base(tmp.path(), true), Platform::Windows, false).unwrap();
        let icon = ProjectIcon::default();
        let opts = AssembleOptions { enable_mcp: true, ..options(&content, &work, &icon) };
        assemble(&mut base, &opts, &out, &no_patch).unwrap();
        assert!(Base::open(&out, Platform::Windows, false).unwrap().mcp_capable);
    }

    #[test]
    fn mcp_needs_an_mcp_package() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work, out) = (dist(&tmp.path().join("dist")), tmp.path().join("work"), tmp.path().join("out.zip"));
        let icon = ProjectIcon::default();
        let opts = AssembleOptions { enable_mcp: true, ..options(&content, &work, &icon) };
        let mut plain = Base::open(&windows_base(tmp.path(), false), Platform::Windows, false).unwrap();
        assert!(!plain.mcp_capable);
        assert!(assemble(&mut plain, &opts, &out, &no_patch).is_err());
        assert!(!out.exists());
        let mut linux = Base::open(&linux_base(tmp.path(), false), Platform::Linux, false).unwrap();
        assert!(!linux.mcp_capable);
        assert!(assemble(&mut linux, &opts, &out, &no_patch).unwrap_err().contains("not a complete MCP-enabled package"));
        let mut macos = Base::open(&macos_base(tmp.path()), Platform::Macos, false).unwrap();
        assert!(!macos.mcp_capable);
        let opts = AssembleOptions { top_folder: "my game.app", ..opts };
        assert!(assemble(&mut macos, &opts, &out, &no_patch).unwrap_err().contains("not a complete MCP-enabled package"));
        assert!(!out.exists());
        // The marker alone (an incomplete mcp/ folder) is not an MCP-enabled package.
        let mut files = Files::new();
        put(&mut files, LINUX_EXE, b"elf", EXEC);
        for file in ["libcef.so", "chrome-sandbox", "icudtl.dat", "resources.pak", "game/index.html", "mcp/enabled"] {
            put(&mut files, file, b"x", FILE_MODE);
        }
        let zip = write_zip(&tmp.path().join("marker-only.zip"), "", &files);
        assert!(!Base::open(&zip, Platform::Linux, false).unwrap().mcp_capable);
    }

    #[test]
    fn linux_mcp_is_kept_next_to_the_executable_or_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work) = (dist(&tmp.path().join("dist")), tmp.path().join("work"));
        let icon = ProjectIcon::default();
        let base_zip = linux_base_with(tmp.path(), false, true);
        let mut base = Base::open(&base_zip, Platform::Linux, false).unwrap();
        assert!(base.mcp_capable);
        let on = tmp.path().join("on.zip");
        let opts = AssembleOptions { enable_mcp: true, ..options(&content, &work, &icon) };
        assemble(&mut base, &opts, &on, &no_patch).unwrap();
        let zip = read_zip(&on);
        for file in MCP_REQUIRED {
            assert!(zip.contains_key(&format!("my game/mcp/{file}")), "{file}");
        }
        assert!(zip.contains_key("my game/mcp/node_modules/.package-lock.json"));
        assert!(Base::open(&on, Platform::Linux, false).unwrap().mcp_capable);

        let off = tmp.path().join("off.zip");
        assemble(&mut base, &options(&content, &work, &icon), &off, &no_patch).unwrap();
        let zip = read_zip(&off);
        assert!(!zip.keys().any(|f| f.starts_with("my game/mcp")), "{:?}", names(&zip));
        assert!(!Base::open(&off, Platform::Linux, false).unwrap().mcp_capable);
    }

    #[test]
    fn macos_mcp_is_kept_in_the_app_resources_or_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work) = (dist(&tmp.path().join("dist")), tmp.path().join("work"));
        let icon = ProjectIcon::default();
        let mut base = Base::open(&macos_base_with(tmp.path(), true), Platform::Macos, false).unwrap();
        assert!(base.mcp_capable);
        let on = tmp.path().join("on.zip");
        let opts = AssembleOptions { enable_mcp: true, top_folder: "my game.app", ..options(&content, &work, &icon) };
        assemble(&mut base, &opts, &on, &no_patch).unwrap();
        let zip = read_zip(&on);
        for file in MCP_REQUIRED {
            assert!(zip.contains_key(&format!("my game.app/Contents/Resources/mcp/{file}")), "{file}");
        }
        assert!(Base::open(&on, Platform::Macos, false).unwrap().mcp_capable);

        let off = tmp.path().join("off.zip");
        let opts = AssembleOptions { top_folder: "my game.app", ..options(&content, &work, &icon) };
        assemble(&mut base, &opts, &off, &no_patch).unwrap();
        let zip = read_zip(&off);
        assert!(!zip.keys().any(|f| f.contains("/mcp")), "{:?}", names(&zip));
        assert!(!Base::open(&off, Platform::Macos, false).unwrap().mcp_capable);
    }

    #[test]
    fn a_marker_added_by_hand_or_a_modified_file_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mut files = windows_files(false);
        with_checksums(&mut files);
        put(&mut files, "mcp/enabled", b"", FILE_MODE);
        let zip = write_zip(&tmp.path().join("marker.zip"), "", &files);
        assert!(Base::open(&zip, Platform::Windows, false).err().unwrap().contains("mcp/enabled"));
        let mut files = windows_files(false);
        with_checksums(&mut files);
        put(&mut files, "play.dll", b"patched", FILE_MODE);
        let zip = write_zip(&tmp.path().join("modified.zip"), "", &files);
        assert!(Base::open(&zip, Platform::Windows, false).err().unwrap().contains("play.dll"));
        let mut files = windows_files(false);
        with_checksums(&mut files);
        files.remove("libcef.dll");
        let zip = write_zip(&tmp.path().join("missing.zip"), "", &files);
        assert!(Base::open(&zip, Platform::Windows, false).is_err());
    }

    #[test]
    fn steam_packages_need_the_redistributable_and_get_the_app_id_next_to_the_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work, out) = (dist(&tmp.path().join("dist")), tmp.path().join("work"), tmp.path().join("out.zip"));
        assert!(Base::open(&linux_base(tmp.path(), false), Platform::Linux, true)
            .err()
            .unwrap()
            .contains("libsteam_api.so"));
        let mut base = Base::open(&linux_base(tmp.path(), true), Platform::Linux, true).unwrap();
        let icon = ProjectIcon::default();
        let opts = AssembleOptions { steam_app_id: Some("480"), ..options(&content, &work, &icon) };
        assemble(&mut base, &opts, &out, &no_patch).unwrap();
        assert_eq!(read_zip(&out)["my game/steam_appid.txt"].0, b"480");

        let mut files = windows_files(false);
        put(&mut files, "steam_api64.dll", b"steam", FILE_MODE);
        with_checksums(&mut files);
        let zip = write_zip(&tmp.path().join("win-steam.zip"), "", &files);
        let mut base = Base::open(&zip, Platform::Windows, true).unwrap();
        let out = tmp.path().join("win.zip");
        assemble(&mut base, &opts, &out, &no_patch).unwrap();
        assert_eq!(read_zip(&out)["my game/steam_appid.txt"].0, b"480");
        Base::open(&out, Platform::Windows, true).unwrap();
    }

    #[test]
    fn linux_keeps_unix_modes_and_places_the_window_icon_next_to_the_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work, out) = (dist(&tmp.path().join("dist")), tmp.path().join("work"), tmp.path().join("out.zip"));
        png(&content.join("icon.png"), 128, [9, 9, 9, 255]);
        let base_zip = linux_base(tmp.path(), false);
        assert_eq!(read_zip(&base_zip)["roves-cef-linux/chrome-sandbox"].1.unwrap() & 0o7777, 0o4755, "fixture");
        let mut base = Base::open(&base_zip, Platform::Linux, false).unwrap();
        let icon = resolve_project_icon(Platform::Linux, None, &content, None, &work).unwrap();
        assert_eq!(icon.summary, "window project (game icon.png 128x128)");
        assemble(&mut base, &options(&content, &work, &icon), &out, &no_patch).unwrap();
        let zip = read_zip(&out);
        let mode = |name: &str| zip[name].1.unwrap() & 0o7777;
        assert_eq!(mode(&format!("my game/{LINUX_EXE}")), 0o755);
        assert_eq!(mode("my game/chrome-sandbox"), 0o4755, "the setuid bit must survive");
        assert_eq!(mode("my game/game/assets/app.js"), 0o644);
        assert_eq!(zip["my game/icon.png"].0, std::fs::read(content.join("icon.png")).unwrap());
        assert!(!names(&zip).iter().any(|f| f.contains("sample")));
    }

    #[test]
    fn macos_replaces_the_game_window_icon_and_bundle_icon_inside_the_app() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work, out) = (dist(&tmp.path().join("dist")), tmp.path().join("work"), tmp.path().join("out.zip"));
        png(&content.join("icon.png"), 256, [200, 30, 30, 255]);
        let mut base = Base::open(&macos_base(tmp.path()), Platform::Macos, false).unwrap();
        let icon = resolve_project_icon(Platform::Macos, None, &content, None, &work).unwrap();
        assert!(icon.summary.ends_with("app icon project (ICNS generated from game icon.png)"), "{}", icon.summary);
        let opts = AssembleOptions { top_folder: "my game.app", ..options(&content, &work, &icon) };
        assemble(&mut base, &opts, &out, &no_patch).unwrap();
        let zip = read_zip(&out);
        assert_eq!(zip["my game.app/Contents/Resources/game/index.html"].0, b"<p>mine</p>");
        assert_eq!(zip["my game.app/Contents/Resources/icon.png"].0, std::fs::read(content.join("icon.png")).unwrap());
        assert_eq!(zip[&format!("my game.app/{MACOS_EXE}")].1.unwrap() & 0o777, 0o755);
        let icns = icns::IconFamily::read(std::io::Cursor::new(&zip[&format!("my game.app/{MACOS_ICNS}")].0)).unwrap();
        assert!(icns.has_icon_with_type(icns::IconType::RGBA32_256x256));
        assert!(zip.contains_key("my game.app/Contents/Frameworks/roves-cef-macos-gate Helper.app/Contents/Info.plist"));

        // No project icon: the sample icon is dropped, the Roves .icns stays.
        let plain = dist(&tmp.path().join("plain"));
        let icon = resolve_project_icon(Platform::Macos, None, &plain, None, &work).unwrap();
        assert!(icon.summary.contains("app icon roves"));
        let out = tmp.path().join("plain.zip");
        let opts = AssembleOptions { top_folder: "my game.app", ..options(&plain, &work, &icon) };
        assemble(&mut base, &opts, &out, &no_patch).unwrap();
        let zip = read_zip(&out);
        assert!(!zip.contains_key("my game.app/Contents/Resources/icon.png"));
        assert_eq!(zip[&format!("my game.app/{MACOS_ICNS}")].0, b"roves icns");
    }

    /// Assembles a Windows package with a recording `patch_exe`; returns the ICO it got.
    fn assemble_windows_with_icon(tmp: &Path, content: &Path, icon: &ProjectIcon) -> (Option<PathBuf>, PathBuf) {
        let mut base = Base::open(&windows_base(tmp, false), Platform::Windows, false).unwrap();
        let (work, out) = (tmp.join("work"), tmp.join("out.zip"));
        let patched = std::cell::RefCell::new(None);
        assemble(&mut base, &options(content, &work, icon), &out, &|exe, ico| {
            assert_eq!(exe, work.join("play.exe"));
            *patched.borrow_mut() = Some(ico.to_path_buf());
            Ok(())
        })
        .unwrap();
        Base::open(&out, Platform::Windows, false).unwrap();
        (patched.into_inner(), out)
    }

    #[test]
    fn windows_icon_precedence_game_png_then_game_ico_then_roves() {
        let tmp = tempfile::tempdir().unwrap();
        let work = tmp.path().join("icons");
        let fallback = ico(&tmp.path().join("roves.ico"), 256, [20, 20, 24, 255]);

        let with_png = dist(&tmp.path().join("png"));
        png(&with_png.join("icon.png"), 512, [200, 30, 30, 255]);
        let icon = resolve_project_icon(Platform::Windows, None, &with_png, Some(&fallback), &work).unwrap();
        assert!(icon.summary.starts_with("window project (game icon.png 512x512); play.exe project"), "{}", icon.summary);
        let (patched, out) = assemble_windows_with_icon(&tmp.path().join("a"), &with_png, &icon);
        assert_eq!(check_ico(&patched.unwrap()), Ok(256));
        assert_eq!(read_zip(&out)["my game/icon.png"].0, std::fs::read(with_png.join("icon.png")).unwrap());
        let provenance = String::from_utf8(read_zip(&out)["my game/PROVENANCE.txt"].0.clone()).unwrap();
        assert!(provenance.contains("icon resource was replaced"));

        let with_favicon = dist(&tmp.path().join("favicon"));
        let favicon = ico(&with_favicon.join("favicon.ico"), 48, [30, 30, 200, 255]);
        let icon = resolve_project_icon(Platform::Windows, None, &with_favicon, Some(&fallback), &work).unwrap();
        let (patched, out) = assemble_windows_with_icon(&tmp.path().join("b"), &with_favicon, &icon);
        assert_eq!(patched.unwrap(), favicon);
        let window = image::load_from_memory(&read_zip(&out)["my game/icon.png"].0).unwrap();
        assert_eq!((window.width(), window.height()), (48, 48));

        let none = dist(&tmp.path().join("none"));
        let icon = resolve_project_icon(Platform::Windows, None, &none, Some(&fallback), &work).unwrap();
        assert!(icon.window_png.is_none() && icon.summary.contains("play.exe roves"));
        let (patched, out) = assemble_windows_with_icon(&tmp.path().join("c"), &none, &icon);
        assert_eq!(patched.unwrap(), fallback);
        assert!(!read_zip(&out).contains_key("my game/icon.png"));

        let icon = resolve_project_icon(Platform::Windows, None, &none, None, &work).unwrap();
        assert!(icon.exe_ico.is_none() && icon.summary.contains("play.exe unchanged"));
    }

    #[test]
    fn the_release_info_icon_wins_on_every_platform_and_must_be_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work) = (dist(&tmp.path().join("dist")), tmp.path().join("work"));
        png(&content.join("icon.png"), 256, [1, 1, 1, 255]);
        let own = png(&tmp.path().join("own.png"), 128, [10, 200, 10, 255]);
        for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
            let icon = resolve_project_icon(platform, Some(&own), &content, None, &work).unwrap();
            assert_eq!(icon.window_png.as_deref(), Some(own.as_path()));
            assert_eq!(icon.exe_ico.is_some(), platform == Platform::Windows);
            assert_eq!(icon.app_icns.is_some(), platform == Platform::Macos);
        }
        let bad = tmp.path().join("bad.png");
        std::fs::write(&bad, b"\x89PNG\r\n\x1a\n not really").unwrap();
        let error = resolve_project_icon(Platform::Linux, Some(&bad), &content, None, &work).unwrap_err();
        assert!(error.starts_with("Release info icon"), "{error}");
    }

    #[test]
    fn malformed_or_out_of_bounds_game_icons_warn_and_fall_back() {
        let tmp = tempfile::tempdir().unwrap();
        let (content, work) = (dist(&tmp.path().join("dist")), tmp.path().join("work"));
        let fallback = ico(&tmp.path().join("roves.ico"), 256, [20, 20, 24, 255]);
        write(&content, "icon.png", b"<html>not an image</html>");
        write(&content, "favicon.ico", &[0, 0, 1, 0, 1, 0].iter().copied().chain([0xff; 30]).collect::<Vec<u8>>());
        let icon = resolve_project_icon(Platform::Windows, None, &content, Some(&fallback), &work).unwrap();
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

    #[test]
    fn the_linux_executable_is_detected_and_steam_libraries_must_match_the_variant() {
        let tmp = tempfile::tempdir().unwrap();
        let base = Base::open(&linux_base(tmp.path(), false), Platform::Linux, false).unwrap();
        assert_eq!(base.executable, LINUX_EXE);
        // A standard asset that ships the Steam library is the wrong asset.
        let steam_zip = linux_base(&tmp.path().join("steam"), true);
        assert!(Base::open(&steam_zip, Platform::Linux, false)
            .err()
            .unwrap()
            .contains("unexpectedly contains libsteam_api.so"));
        assert!(Base::open(&steam_zip, Platform::Linux, true).is_ok());
        // Unix modes lost (an archive re-zipped on Windows): no executable to find.
        let mut files = Files::new();
        for file in [LINUX_EXE, "libcef.so", "chrome-sandbox", "icudtl.dat", "resources.pak", "game/index.html"] {
            put(&mut files, file, b"x", FILE_MODE);
        }
        let zip = write_zip(&tmp.path().join("modes.zip"), "", &files);
        assert!(Base::open(&zip, Platform::Linux, false).err().unwrap().contains("Unix modes"));
    }

    #[test]
    fn macos_names_come_from_info_plist() {
        assert_eq!(
            macos_bundle_names(MACOS_PLIST.as_bytes()).unwrap(),
            (MACOS_EXE.to_string(), MACOS_ICNS.to_string())
        );
        let with_extension = MACOS_PLIST.replace("<string>roves-cef-macos-gate</string>
</dict>", "<string>x.icns</string>
</dict>");
        assert_eq!(macos_bundle_names(with_extension.as_bytes()).unwrap().1, "Contents/Resources/x.icns");
        assert!(macos_bundle_names(b"<plist version=\"1.0\"><dict/></plist>").is_err());
    }

    /// A real PE with its own icon group, version info and manifest (a Windows system
    /// executable copied to a temp dir): the main icon group is replaced by the generated ICO,
    /// the other resources are kept, and the patched copy still runs.
    #[cfg(windows)]
    #[test]
    fn set_exe_icon_replaces_the_main_icon_group_of_a_real_executable() {
        let system = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
        let source = system.join("System32").join("cmd.exe");
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("play.exe");
        std::fs::copy(&source, &exe).unwrap();
        let before = editpe::Image::parse_file(&exe).unwrap();
        let before = before.resource_directory().unwrap().clone();
        let old_icon = before.get_main_icon().unwrap().map(<[u8]>::to_vec);
        let version = before.get_version_info().unwrap();
        let manifest = before.get_manifest().unwrap();
        assert!(old_icon.is_some() && version.is_some(), "the test executable needs an icon and version info");

        let icon = ico(&tmp.path().join("game.ico"), 256, [200, 30, 30, 255]);
        set_exe_icon(&exe, &icon).unwrap();

        let after = editpe::Image::parse_file(&exe).unwrap();
        let after = after.resource_directory().unwrap();
        let new_icon = after.get_main_icon().unwrap().unwrap().to_vec();
        assert_ne!(Some(new_icon.clone()), old_icon);
        // The main icon is now the first frame of the generated ICO (its image data).
        let ico_bytes = std::fs::read(&icon).unwrap();
        let first_offset = u32::from_le_bytes(ico_bytes[18..22].try_into().unwrap()) as usize;
        let first_size = u32::from_le_bytes(ico_bytes[14..18].try_into().unwrap()) as usize;
        assert_eq!(new_icon, ico_bytes[first_offset..first_offset + first_size]);
        // Exactly one group: the old main group is gone, not left behind for Explorer.
        let groups = after.root().get(editpe::ResourceEntryName::ID(14)).unwrap().as_table().unwrap();
        assert_eq!(groups.entries().len(), 1, "{:?}", groups.entries());
        assert_eq!(after.get_version_info().unwrap(), version);
        assert_eq!(after.get_manifest().unwrap(), manifest);
        let status = std::process::Command::new(&exe).args(["/d", "/c", "exit 7"]).status().unwrap();
        assert_eq!(status.code(), Some(7), "the patched copy must still run");
    }

    /// Manual check against a real engine Windows package (an extracted
    /// `visual-novel-template-roves-cef-windows-x64` folder or release zip contents):
    /// `ROVES_REAL_WINDOWS_PACKAGE=<folder> cargo test --lib real_windows_package -- --ignored --nocapture`.
    /// The folder is never modified: everything runs on copies in a temp dir.
    #[cfg(windows)]
    #[test]
    #[ignore = "needs ROVES_REAL_WINDOWS_PACKAGE (a real engine package, ~450 MB)"]
    fn real_windows_package() {
        let package = PathBuf::from(std::env::var_os("ROVES_REAL_WINDOWS_PACKAGE").expect("ROVES_REAL_WINDOWS_PACKAGE"));
        let tmp = tempfile::tempdir().unwrap();
        let resources = |exe: &Path| editpe::Image::parse_file(exe).unwrap().resource_directory().unwrap().clone();
        let group_count = |dir: &editpe::ResourceDirectory| {
            dir.root().get(editpe::ResourceEntryName::ID(14)).unwrap().as_table().unwrap().entries().len()
        };
        let original = resources(&package.join("play.exe"));
        let (version, manifest) = (original.get_version_info().unwrap(), original.get_manifest().unwrap());
        let sample_icon = original.get_main_icon().unwrap().map(<[u8]>::to_vec);
        eprintln!("original: groups={} version={} manifest={}", group_count(&original), version.is_some(), manifest.is_some());

        // 1. set_exe_icon on a copy of play.exe, with roves.ico and with a generated ICO.
        let generated = ico(&tmp.path().join("generated.ico"), 256, [200, 30, 30, 255]);
        for ico_path in [package.join("roves.ico"), generated] {
            let exe = tmp.path().join("play-copy.exe");
            std::fs::copy(package.join("play.exe"), &exe).unwrap();
            set_exe_icon(&exe, &ico_path).unwrap();
            let after = resources(&exe);
            let ico_bytes = std::fs::read(&ico_path).unwrap();
            let size = u32::from_le_bytes(ico_bytes[14..18].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(ico_bytes[18..22].try_into().unwrap()) as usize;
            let main = after.get_main_icon().unwrap().unwrap().to_vec();
            assert_eq!(main, ico_bytes[offset..offset + size], "{}", ico_path.display());
            assert_ne!(Some(main), sample_icon);
            assert_eq!(group_count(&after), 1);
            assert_eq!(after.get_version_info().unwrap(), version);
            assert_eq!(after.get_manifest().unwrap(), manifest);
            eprintln!("set_exe_icon with {}: ok", ico_path.display());
        }

        // Runs `play.exe` of an assembled folder and returns its log.
        let run = |dir: &Path, name: &str| {
            let log = tmp.path().join(format!("{name}.log"));
            let status = std::process::Command::new(dir.join("play.exe"))
                .arg("--roves-isolated-profile")
                .arg("--roves-auto-close-ms=8000")
                .arg("--roves-no-error-dialog")
                .arg(format!("--roves-log={}", log.display()))
                .status()
                .unwrap();
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            assert_eq!(status.code(), Some(0), "{name}: {status}\n{text}");
            assert!(text.contains("startup released"), "{name}: no `startup released`\n{text}");
            eprintln!("{name}: exit 0, startup released");
            text
        };

        // 2. The whole package copied, its play.exe patched, run.
        let copy = tmp.path().join("copy");
        for entry in walkdir::WalkDir::new(&package) {
            let entry = entry.unwrap();
            let target = copy.join(entry.path().strip_prefix(&package).unwrap());
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&target).unwrap();
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
        set_exe_icon(&copy.join("play.exe"), &package.join("roves.ico")).unwrap();
        run(&copy, "patched-copy");

        // 3. Full assembly: zip the package as a release asset would be, assemble a tiny game
        //    with its own icon, verify the output's checksums, extract and run it.
        let asset = tmp.path().join("roves_shell_windows.zip");
        {
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&asset).unwrap());
            let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
            for entry in walkdir::WalkDir::new(&package).min_depth(1) {
                let entry = entry.unwrap();
                let rel = entry.path().strip_prefix(&package).unwrap().to_string_lossy().replace('\\', "/");
                let name = format!("visual-novel-template-roves-cef/{rel}");
                if entry.file_type().is_dir() {
                    writer.add_directory(format!("{name}/"), stored).unwrap();
                } else {
                    writer.start_file(name, stored).unwrap();
                    std::io::copy(&mut std::fs::File::open(entry.path()).unwrap(), &mut writer).unwrap();
                }
            }
            writer.finish().unwrap();
        }
        let mut base = Base::open(&asset, Platform::Windows, false).unwrap();
        let game = tmp.path().join("game");
        write(&game, "index.html", b"<!doctype html><title>tiny</title><body style=\"background:#c33\"><h1>tiny game</h1></body>");
        png(&game.join("icon.png"), 256, [200, 30, 30, 255]);
        let work = tmp.path().join("work");
        let fallback = base.fallback_ico(&work.join("icon")).unwrap();
        let icon = resolve_project_icon(Platform::Windows, None, &game, fallback.as_deref(), &work.join("icon")).unwrap();
        eprintln!("icon: {}", icon.summary);
        let out = tmp.path().join("tiny_windows.zip");
        let opts = AssembleOptions { top_folder: "tiny", ..options(&game, &work, &icon) };
        assemble(&mut base, &opts, &out, &set_exe_icon).unwrap();
        let assembled = Base::open(&out, Platform::Windows, false).unwrap();
        assert!(!assembled.mcp_capable);
        eprintln!("assembled package: SHA256SUMS.txt verified ({} entries)", assembled.entries.len());
        let extracted = tmp.path().join("extracted");
        zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap().extract(&extracted).unwrap();
        let root = extracted.join("tiny");
        assert_eq!(std::fs::read(root.join("game/index.html")).unwrap(), std::fs::read(game.join("index.html")).unwrap());
        let mut game_files: Vec<String> = walkdir::WalkDir::new(root.join("game"))
            .into_iter()
            .map(Result::unwrap)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        game_files.sort();
        assert_eq!(game_files, ["icon.png", "index.html"], "sample game files left over");
        assert_eq!(std::fs::read(root.join("icon.png")).unwrap(), std::fs::read(game.join("icon.png")).unwrap());
        run(&root, "assembled");
    }

    #[test]
    fn content_without_index_html_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let content = tmp.path().join("dist");
        write(&content, "main.js", b"x");
        let mut base = Base::open(&windows_base(tmp.path(), false), Platform::Windows, false).unwrap();
        let icon = ProjectIcon::default();
        let work = tmp.path().join("work");
        assert!(assemble(&mut base, &options(&content, &work, &icon), &tmp.path().join("out.zip"), &no_patch).is_err());
    }
}
