//! Wraps an already-assembled portable bundle (see `bundle.rs`'s `staging_dir`) into a
//! real, installable package: `.msi` (WiX), `.dmg` (`hdiutil`), `.deb` (`dpkg-deb`) —
//! porting `python/servo/post_build_commands.py`'s `_wrap_windows_msi`/`_wrap_macos_dmg`/
//! `_bundle_linux_deb` (and `support/windows/roves-bundle.wxs.mako` for the WiX source) to
//! Rust line-for-line where practical. Unlike the portable/shell-download path, there's no
//! way to produce these without the matching native tool already installed on the machine
//! running Packmaster — see `check_installer_availability`, which is a real, honest check
//! (host OS + tool-on-PATH), not an assumption.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

/// `windows` | `linux` | `macos` — the OS Packmaster itself is currently running on. An
/// installer can only ever be built for this one platform: unlike the portable shell
/// (downloaded prebuilt, so any host can produce any platform's bundle), installer-wrapping
/// shells out to that platform's own native tool (WiX/`hdiutil`/`dpkg-deb`), which only
/// exists on its own OS.
pub fn host_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn tool_on_path(tool: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    let candidates: Vec<String> = if cfg!(windows) {
        [".exe", ".cmd", ".bat"].iter().map(|ext| format!("{tool}{ext}")).collect()
    } else {
        vec![tool.to_string()]
    };
    std::env::split_paths(&path_var)
        .any(|dir| candidates.iter().any(|name| dir.join(name).is_file()))
}

/// WiX v3's own installer doesn't reliably put `candle`/`light` on `PATH` — many installs
/// only set the `WIX` environment variable to the install root (see the engine repo's own
/// `.github/workflows/test.yml`, which has to add `$env:WIX\bin` to `PATH` itself for the
/// exact same reason).
fn wix_tool_available(tool: &str) -> bool {
    if tool_on_path(tool) {
        return true;
    }
    std::env::var_os("WIX")
        .map(|wix| Path::new(&wix).join("bin").join(format!("{tool}.exe")).is_file())
        .unwrap_or(false)
}

/// Real feasibility check for one installer format on this host — confirms both that this
/// is the right OS for it and that the native tool it needs is actually installed, instead
/// of just assuming. Returns `(available, reason_if_not)`.
pub fn check_installer_availability(platform: &str) -> (bool, Option<String>) {
    if platform != host_platform() {
        return (
            false,
            Some(format!("only buildable when Packmaster is running on {platform}")),
        );
    }
    match platform {
        "windows" => {
            if wix_tool_available("candle") && wix_tool_available("light") {
                (true, None)
            } else {
                (false, Some("requires the WiX Toolset (candle/light) to be installed".to_string()))
            }
        }
        "macos" => {
            if tool_on_path("hdiutil") {
                (true, None)
            } else {
                (false, Some("requires hdiutil".to_string()))
            }
        }
        "linux" => {
            if tool_on_path("dpkg-deb") {
                (true, None)
            } else {
                (false, Some("requires dpkg-deb (from dpkg-dev)".to_string()))
            }
        }
        _ => (false, None),
    }
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let output = cmd.output().map_err(|e| format!("running {program}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ── Windows: .msi via WiX (candle/light) ────────────────────────────────────────────────

/// WiX's `Product/@Version` must be a dotted run of 1-4 integers, each 0-65535 — an MSI
/// file-format constraint. Strips a leading `v` (a common git tag convention).
fn sanitize_msi_version(version: &str) -> Result<String, String> {
    let trimmed = if version.to_ascii_lowercase().starts_with('v')
        && version.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
    {
        &version[1..]
    } else {
        version
    };
    let parts: Vec<&str> = trimmed.split('.').collect();
    let valid = !parts.is_empty()
        && parts.len() <= 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && p.parse::<u32>().is_ok_and(|n| n <= 65535));
    if !valid {
        return Err(format!(
            "version {version:?} isn't a valid MSI version (1-4 dot-separated integers, each 0-65535, e.g. 1.2.3) — required for an .msi installer"
        ));
    }
    Ok(trimmed.to_string())
}

fn make_wix_id(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if matches!(c, '-' | '/' | '\\' | '.') { '_' } else { c })
        .collect();
    format!("Id{cleaned}")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Recursively harvests `stage_dir` into WiX `<Directory>`/`<Component>`/`<File>` elements —
/// one Component per directory (mirroring `roves-bundle.wxs.mako`'s `include_directory`),
/// collecting their IDs so the caller can add matching `<ComponentRef>`s.
fn wix_include_directory(stage_dir: &Path, dir: &Path, name: &str, components: &mut Vec<String>) -> Result<String, String> {
    let dir_name = dir
        .file_name()
        .ok_or_else(|| format!("{dir:?} has no file name"))?
        .to_string_lossy()
        .into_owned();
    let id = make_wix_id(&dir_name);
    components.push(id.clone());

    let mut files_xml = String::new();
    let mut subdirs_xml = String::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let rel = path.strip_prefix(stage_dir).map_err(|e| e.to_string())?.to_string_lossy().into_owned();
            files_xml.push_str(&format!(
                "<File Id=\"{}\" Name=\"{}\" Source=\"{}\" DiskId=\"1\"/>\n",
                make_wix_id(&rel),
                xml_escape(&file_name),
                xml_escape(&path.to_string_lossy())
            ));
        }
    }
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            let sub_name = entry.file_name().to_string_lossy().into_owned();
            subdirs_xml.push_str(&wix_include_directory(stage_dir, &path, &sub_name, components)?);
        }
    }

    Ok(format!(
        "<Directory Id=\"{id}\" Name=\"{}\">\n<Component Id=\"{id}\" Guid=\"{}\" Win64=\"yes\">\n<CreateFolder/>\n{files_xml}</Component>\n{subdirs_xml}</Directory>\n",
        xml_escape(name),
        Uuid::new_v4()
    ))
}

pub fn build_msi(stage_dir: &Path, output_dir: &Path, package_name: &str, version: &str) -> Result<PathBuf, String> {
    if !(wix_tool_available("candle") && wix_tool_available("light")) {
        return Err("--msi requires the WiX Toolset (candle/light not found)".to_string());
    }
    let msi_version = sanitize_msi_version(version)?;
    // Deterministic per-package-name (not per-build): WiX's MajorUpgrade mechanism uses a
    // stable UpgradeCode to recognize "a newer version of the same product" across separate
    // generations — a fresh random one every time would make every install look unrelated.
    let upgrade_code = Uuid::new_v5(&Uuid::NAMESPACE_DNS, format!("roves.bundle.{package_name}").as_bytes());

    let mut flat_files = String::new();
    for entry in fs::read_dir(stage_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "play.exe" {
                continue;
            }
            flat_files.push_str(&format!(
                "<File Id=\"{}\" Name=\"{}\" Source=\"{}\" DiskId=\"1\"/>\n",
                make_wix_id(&name),
                xml_escape(&name),
                xml_escape(&entry.path().to_string_lossy())
            ));
        }
    }

    let mut components = Vec::new();
    let mut dirs_xml = String::new();
    for entry in fs::read_dir(stage_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            dirs_xml.push_str(&wix_include_directory(stage_dir, &entry.path(), &name, &mut components)?);
        }
    }
    let component_refs: String =
        components.iter().map(|c| format!("<ComponentRef Id=\"{c}\"/>\n")).collect();

    let play_exe = xml_escape(&stage_dir.join("play.exe").to_string_lossy());
    let escaped_name = xml_escape(package_name);
    let main_guid = Uuid::new_v4();
    let menu_guid = Uuid::new_v4();

    let wxs = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="{escaped_name}" Manufacturer="unspecified" UpgradeCode="{upgrade_code}" Language="1033" Codepage="1252" Version="{msi_version}">
    <Package Id="*" Keywords="Installer" Description="{escaped_name} installer" Manufacturer="unspecified" InstallerVersion="200" Platform="x64" Languages="1033" SummaryCodepage="1252" Compressed="yes"/>
    <MajorUpgrade AllowDowngrades="yes"/>
    <Media Id="1" Cabinet="bundle.cab" EmbedCab="yes"/>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder" Name="PFiles">
        <Directory Id="INSTALLDIR" Name="{escaped_name}">
          <Component Id="MainExe" Guid="{main_guid}" Win64="yes">
            <File Id="PlayEXE" Name="play.exe" DiskId="1" Source="{play_exe}" KeyPath="yes"/>
            {flat_files}
          </Component>
          {dirs_xml}
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder" Name="Programs">
        <Directory Id="ProgramMenuDir" Name="{escaped_name}">
          <Component Id="ProgramMenuDir" Guid="{menu_guid}">
            <RemoveFolder Id="ProgramMenuDir" On="both"/>
            <RegistryValue Root="HKCU" Key="Software\{escaped_name}" Type="string" Value="" KeyPath="yes"/>
            <Shortcut Id="StartMenuShortcut" Directory="ProgramMenuDir" Name="{escaped_name}" Target="[INSTALLDIR]play.exe" WorkingDirectory="INSTALLDIR" Icon="PlayIcon"/>
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="Complete" Level="1">
      <ComponentRef Id="MainExe"/>
      {component_refs}
      <ComponentRef Id="ProgramMenuDir"/>
    </Feature>
    <Icon Id="PlayIcon" SourceFile="{play_exe}"/>
  </Product>
</Wix>
"#
    );

    let msi_build_dir = output_dir.join("_msi-build");
    if msi_build_dir.exists() {
        fs::remove_dir_all(&msi_build_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&msi_build_dir).map_err(|e| e.to_string())?;
    fs::write(msi_build_dir.join("Bundle.wxs"), wxs).map_err(|e| e.to_string())?;

    run(Command::new("candle").arg("Bundle.wxs").current_dir(&msi_build_dir))?;
    run(Command::new("light").arg("Bundle.wixobj").current_dir(&msi_build_dir))?;

    let msi_path = output_dir.join(format!("{package_name}_{version}.msi"));
    if msi_path.exists() {
        fs::remove_file(&msi_path).map_err(|e| e.to_string())?;
    }
    fs::rename(msi_build_dir.join("Bundle.msi"), &msi_path).map_err(|e| e.to_string())?;
    fs::remove_dir_all(&msi_build_dir).ok();
    Ok(msi_path)
}

// ── macOS: .dmg via hdiutil ──────────────────────────────────────────────────────────────

pub fn build_dmg(stage_dir: &Path, output_dir: &Path, package_name: &str, version: &str) -> Result<PathBuf, String> {
    if !tool_on_path("hdiutil") {
        return Err("--dmg requires hdiutil, which was not found".to_string());
    }
    let applications_link = stage_dir.join("Applications");
    if !applications_link.exists() {
        #[cfg(unix)]
        std::os::unix::fs::symlink("/Applications", &applications_link).map_err(|e| e.to_string())?;
    }
    let dmg_path = output_dir.join(format!("{package_name}-{version}.dmg"));
    if dmg_path.exists() {
        fs::remove_file(&dmg_path).map_err(|e| e.to_string())?;
    }
    run(Command::new("hdiutil")
        .args(["create", "-volname", package_name, "-srcfolder"])
        .arg(stage_dir)
        .arg(&dmg_path))?;
    Ok(dmg_path)
}

// ── Linux: .deb via dpkg-deb ─────────────────────────────────────────────────────────────

pub fn build_deb(stage_dir: &Path, output_dir: &Path, package_name: &str, version: &str) -> Result<PathBuf, String> {
    if !tool_on_path("dpkg-deb") {
        return Err("--deb requires dpkg-deb (from dpkg-dev), which was not found".to_string());
    }
    let debian_arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "i386",
        other => other,
    };

    let pkg_root = output_dir.join("pkgroot");
    if pkg_root.exists() {
        fs::remove_dir_all(&pkg_root).map_err(|e| e.to_string())?;
    }
    let lib_dir = pkg_root.join("usr").join("lib").join(package_name);
    let bin_dir = pkg_root.join("usr").join("bin");
    let applications_dir = pkg_root.join("usr").join("share").join("applications");
    let debian_dir = pkg_root.join("DEBIAN");
    for dir in [&lib_dir, &bin_dir, &applications_dir, &debian_dir] {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }

    // stage_dir is already exactly the flat portable layout (binary + .so deps + packed
    // content + launch.json) that /usr/lib/<package_name>/ should contain — no need to
    // re-place content from scratch, just copy what's already there.
    copy_dir_recursive(stage_dir, &lib_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let play_path = lib_dir.join("play");
        if play_path.exists() {
            fs::set_permissions(&play_path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        }
    }

    // /usr/lib/<package_name> isn't on PATH, so /usr/bin/<package_name> needs to point at
    // the real binary — a symlink, not a wrapper script: the engine finds its own .so
    // dependencies via the $ORIGIN rpath and resolves its own launch args in-process.
    #[cfg(unix)]
    std::os::unix::fs::symlink(Path::new("/usr/lib").join(package_name).join("play"), bin_dir.join(package_name))
        .map_err(|e| e.to_string())?;

    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName={package_name}\nExec=/usr/bin/{package_name}\nTerminal=false\nCategories=Network;WebBrowser;\n"
    );
    fs::write(applications_dir.join(format!("{package_name}.desktop")), desktop_entry).map_err(|e| e.to_string())?;

    // lstat-equivalent sizes, not following symlinks: usr/bin/<package_name>'s target is an
    // absolute path that only exists after real installation, not on this build machine.
    let installed_size_kb: u64 = walkdir::WalkDir::new(&pkg_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_type().is_dir())
        .filter_map(|e| fs::symlink_metadata(e.path()).ok())
        .map(|m| m.len())
        .sum::<u64>()
        / 1024;

    let control = format!(
        "Package: {package_name}\nVersion: {version}\nSection: web\nPriority: optional\nArchitecture: {debian_arch}\nInstalled-Size: {installed_size_kb}\nMaintainer: unspecified <unspecified@example.com>\nDescription: {package_name} (Roves-based application bundle)\n Packaged by Roves Packmaster.\n"
    );
    fs::write(debian_dir.join("control"), control).map_err(|e| e.to_string())?;

    let deb_path = output_dir.join(format!("{package_name}_{version}_{debian_arch}.deb"));
    run(Command::new("dpkg-deb").args(["--build", "--root-owner-group"]).arg(&pkg_root).arg(&deb_path))?;
    fs::remove_dir_all(&pkg_root).ok();
    Ok(deb_path)
}
