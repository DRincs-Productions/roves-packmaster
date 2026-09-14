//! Real iOS app generation for Packmaster: downloads the `support/ios/` WKWebView container
//! (`App.swift`) plus its branding assets (published by the engine's own
//! `.github/workflows/ios.yml` to the rolling "test" GitHub Release, alongside
//! `resources/servo_1024.png`/the Metal Mania font -- see that workflow's own comment; iOS
//! isn't part of a real, tagged release yet, same reasoning as `android.rs`'s own
//! `ANDROID_TEST_RELEASE_TAG`), stages a WKWebView Xcode project from the user's own content,
//! and builds it with XcodeGen + `xcodebuild` -- all auto-downloaded and cached, exactly like
//! `android.rs` does for the Android Gradle project.
//!
//! **Only available when Packmaster itself is running on macOS** (`check_ios_availability`) --
//! unlike Android, there is no way around this: Xcode/XcodeGen are Apple-only tools, with no
//! cross-platform equivalent. Mirrors the engine's own `support/ios/bundle.py`'s `stage()` for
//! the project-staging step, and `python/servo/post_build_commands.py`'s `_bundle_ios`/
//! `_sign_and_export_ios_release` (added 2026-09-14, see the engine repo's own
//! CUSTOMIZATIONS.md) for the unsigned-build/signed-release-export steps -- see those
//! functions' own doc comments for the design this ports to Rust.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::android::{download_with_retries, extract_zip, run_capturing_output};
use crate::ios_signing::IosSigningConfig;

/// Where `.github/workflows/ios.yml` (the main engine repo, DRincs-Productions/roves)
/// publishes `roves_ios_project.zip` -- same rolling "test" release/repo `android.rs`'s own
/// `ANDROID_TEST_RELEASE_TAG`/`ANDROID_REPO` already target, for the identical reason (iOS is
/// still experimental, see the engine README's "Supported platforms" table -- nothing
/// versioned to pin to yet).
const IOS_TEST_RELEASE_TAG: &str = "test";
const IOS_REPO: &str = "DRincs-Productions/roves";

fn test_release_asset_url(asset_name: &str) -> String {
    format!("https://github.com/{IOS_REPO}/releases/download/{IOS_TEST_RELEASE_TAG}/{asset_name}")
}

fn tools_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("ios-tools"))
}

/// Real feasibility check, mirroring `android::check_android_availability`'s own
/// `(available, reason_if_not)` shape. Unlike Android, this is a genuine, permanent
/// restriction, not a historical gap that later closed -- Xcode/XcodeGen simply don't exist
/// outside macOS.
pub fn check_ios_availability() -> (bool, Option<String>) {
    if !cfg!(target_os = "macos") {
        return (
            false,
            Some(
                "iOS packaging requires running Packmaster on macOS -- Xcode and XcodeGen are Apple-only tools with no cross-platform equivalent."
                    .to_string(),
            ),
        );
    }
    if std::process::Command::new("xcodebuild").arg("-version").output().is_err() {
        return (false, Some("Xcode's command-line tools aren't installed -- run `xcode-select --install`.".to_string()));
    }
    if std::process::Command::new("xcodegen").arg("--version").output().is_err() {
        return (false, Some("XcodeGen isn't installed -- run `brew install xcodegen`.".to_string()));
    }
    (true, None)
}

/// Downloads (once per app run, same "don't cache a rolling release" reasoning as
/// `android.rs`'s own `download_android_project`) the engine's `support/ios/` container +
/// `resources/` branding assets.
async fn download_ios_project(app: &AppHandle, mut on_progress: impl FnMut(f64) + Send) -> Result<PathBuf, String> {
    let dest_dir = tools_cache_dir(app)?.join("ios-project");
    if dest_dir.exists() {
        tokio::fs::remove_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;

    let zip_path = dest_dir.join("project.zip");
    download_with_retries(&test_release_asset_url("roves_ios_project.zip"), &zip_path, &mut on_progress)
        .await
        .map_err(|e| format!("downloading the iOS project: {e}"))?;
    extract_zip(&zip_path, &dest_dir)?;
    tokio::fs::remove_file(&zip_path).await.ok();
    // The zip's own root is `support/ios/...` + `resources/...` (see ios.yml's own packaging
    // step) -- exactly the two relative paths this function's caller expects.
    Ok(dest_dir)
}

pub struct IosBuildOptions<'a> {
    pub content_dir: &'a Path,
    pub app_name_override: &'a str,
    pub bundle_id_override: &'a str,
    /// `Some` archives and exports a real, signed `.ipa` instead of the default unsigned
    /// Simulator `.app` -- mirrors the engine's own `mach bundle --ios --ios-release`, see
    /// `ios_signing.rs`'s own doc comment for the full mechanism.
    pub signing: Option<&'a IosSigningConfig>,
}

fn app_name_or_default(app_name_override: &str) -> String {
    if app_name_override.trim().is_empty() { "Roves Game".to_string() } else { app_name_override.trim().to_string() }
}

fn bundle_id_or_default(bundle_id_override: &str) -> String {
    if bundle_id_override.trim().is_empty() { "org.roves.game".to_string() } else { bundle_id_override.trim().to_string() }
}

/// Ports `support/ios/bundle.py`'s `stage()` to Rust: copies `App.swift` + the user's content
/// + branding assets into a fresh Xcode-project scaffold, and writes `Info.plist`/
/// `project.json`. Returns the staged project directory.
fn stage_project(project_root: &Path, content_dir: &Path, staging_dir: &Path, app_name: &str, bundle_id: &str) -> Result<(), String> {
    if !content_dir.join("index.html").is_file() {
        return Err("iOS bundles require your content directory to contain an index.html.".to_string());
    }
    if staging_dir.exists() {
        std::fs::remove_dir_all(staging_dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(staging_dir).map_err(|e| e.to_string())?;

    std::fs::copy(project_root.join("support").join("ios").join("App.swift"), staging_dir.join("App.swift"))
        .map_err(|e| e.to_string())?;
    crate::android::copy_dir_recursive(content_dir, &staging_dir.join("www"))?;

    let brand_dir = staging_dir.join("roves-brand");
    std::fs::create_dir_all(&brand_dir).map_err(|e| e.to_string())?;
    let resources_dir = project_root.join("resources");
    std::fs::copy(resources_dir.join("servo_1024.png"), brand_dir.join("servo_1024.png")).map_err(|e| e.to_string())?;
    std::fs::copy(resources_dir.join("fonts").join("MetalMania-Regular.ttf"), brand_dir.join("MetalMania-Regular.ttf"))
        .map_err(|e| e.to_string())?;
    std::fs::copy(resources_dir.join("fonts").join("MetalMania-OFL.txt"), brand_dir.join("MetalMania-OFL.txt"))
        .map_err(|e| e.to_string())?;

    // Mirrors bundle.py's own `info` dict exactly -- `CFBundleIdentifier`/`CFBundleExecutable`/
    // `CFBundleName` stay as Xcode build-setting variables ($(...)), substituted at build
    // time, not by this staging step.
    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key>
    <string>{app_name}</string>
    <key>CFBundleIdentifier</key>
    <string>$(PRODUCT_BUNDLE_IDENTIFIER)</string>
    <key>CFBundleExecutable</key>
    <string>$(EXECUTABLE_NAME)</string>
    <key>CFBundleName</key>
    <string>$(PRODUCT_NAME)</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>UILaunchScreen</key>
    <dict/>
    <key>UISupportedInterfaceOrientations</key>
    <array>
        <string>UIInterfaceOrientationPortrait</string>
        <string>UIInterfaceOrientationLandscapeLeft</string>
        <string>UIInterfaceOrientationLandscapeRight</string>
    </array>
    <key>UIViewControllerBasedStatusBarAppearance</key>
    <true/>
    <key>UIStatusBarHidden</key>
    <true/>
</dict>
</plist>
"#
    );
    std::fs::write(staging_dir.join("Info.plist"), info_plist).map_err(|e| e.to_string())?;

    let project_json = serde_json::json!({
        "name": "RovesGame",
        "options": { "deploymentTarget": { "iOS": "15.0" } },
        "targets": {
            "RovesGame": {
                "type": "application",
                "platform": "iOS",
                "sources": [
                    { "path": "App.swift" },
                    { "path": "www", "type": "folder", "buildPhase": "resources" },
                    { "path": "roves-brand", "type": "folder", "buildPhase": "resources" },
                ],
                "settings": {
                    "base": {
                        "PRODUCT_BUNDLE_IDENTIFIER": bundle_id,
                        "INFOPLIST_FILE": "Info.plist",
                        "SWIFT_VERSION": "5.0",
                        "TARGETED_DEVICE_FAMILY": "1,2",
                    }
                }
            }
        }
    });
    std::fs::write(staging_dir.join("project.json"), serde_json::to_string_pretty(&project_json).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Builds an unsigned iOS Simulator `.app` (or, with `options.signing`, a signed `.ipa`) and
/// returns its path. `on_progress(phase, fraction)` mirrors `android::build_apk`'s own shape,
/// with `"ios"` as the pseudo-platform name.
pub async fn build_ios(
    app: &AppHandle,
    options: &IosBuildOptions<'_>,
    mut on_progress: impl FnMut(&str, f64) + Send,
) -> Result<PathBuf, String> {
    let (available, reason) = check_ios_availability();
    if !available {
        return Err(reason.unwrap_or_else(|| "iOS packaging isn't available on this machine".to_string()));
    }

    on_progress("downloading-ios-project", 0.0);
    let project_root = download_ios_project(app, |f| on_progress("downloading-ios-project", f)).await?;

    on_progress("staging", 0.0);
    let staging_dir = std::env::temp_dir().join(format!("roves-packmaster-ios-{}", std::process::id()));
    let app_name = app_name_or_default(options.app_name_override);
    let bundle_id = bundle_id_or_default(options.bundle_id_override);
    stage_project(&project_root, options.content_dir, &staging_dir, &app_name, &bundle_id)?;

    on_progress("generating-project", 0.3);
    run_capturing_output(
        std::process::Command::new("xcodegen").current_dir(&staging_dir).arg("generate").arg("--spec").arg("project.json"),
        "xcodegen generate",
    )?;

    if let Some(signing) = options.signing {
        on_progress("signing", 0.5);
        let ipa_path = sign_and_export(&staging_dir, &bundle_id, signing)?;
        on_progress("done", 1.0);
        return Ok(ipa_path);
    }

    on_progress("building", 0.5);
    let build_dir = staging_dir.join("build");
    run_capturing_output(
        std::process::Command::new("xcodebuild")
            .current_dir(&staging_dir)
            .args(["-project", "RovesGame.xcodeproj", "-scheme", "RovesGame"])
            .args(["-configuration", "Debug", "-sdk", "iphonesimulator"])
            .arg("-derivedDataPath")
            .arg(&build_dir)
            .args(["CODE_SIGN_IDENTITY=", "CODE_SIGNING_REQUIRED=NO", "CODE_SIGNING_ALLOWED=NO", "build"]),
        "xcodebuild (unsigned, iphonesimulator)",
    )?;
    let products_dir = build_dir.join("Build").join("Products").join("Debug-iphonesimulator");
    let app_path = products_dir.join("RovesGame.app");
    if !app_path.is_dir() {
        return Err(format!("expected {app_path:?} after the Simulator build, but it doesn't exist"));
    }
    on_progress("done", 1.0);
    Ok(app_path)
}

/// Ports the engine's `_sign_and_export_ios_release` (`python/servo/post_build_commands.py`)
/// to Rust -- see that function's own doc comment for the full mechanism (an ephemeral
/// keychain, never the caller's login one; the provisioning profile installed by its own
/// Apple-assigned UUID; `xcodebuild archive` + `-exportArchive`). Kept in its own function so
/// `build_ios`'s signed/unsigned branches each stay readable on their own.
fn sign_and_export(staging_dir: &Path, bundle_id: &str, signing: &IosSigningConfig) -> Result<PathBuf, String> {
    let keychain_path = staging_dir.join("roves-ios-signing.keychain");
    let keychain_password = uuid::Uuid::new_v4().to_string();

    let result: Result<PathBuf, String> = (|| {
        run_capturing_output(
            std::process::Command::new("security").args(["create-keychain", "-p", keychain_password.as_str()]).arg(&keychain_path),
            "security create-keychain",
        )?;
        run_capturing_output(
            std::process::Command::new("security").args(["set-keychain-settings", "-lut", "3600"]).arg(&keychain_path),
            "security set-keychain-settings",
        )?;
        run_capturing_output(
            std::process::Command::new("security").args(["unlock-keychain", "-p", keychain_password.as_str()]).arg(&keychain_path),
            "security unlock-keychain",
        )?;
        run_capturing_output(
            std::process::Command::new("security")
                .arg("import")
                .arg(&signing.certificate_p12_path)
                .arg("-k")
                .arg(&keychain_path)
                .args(["-P", signing.certificate_p12_password.as_str()])
                .args(["-T", "/usr/bin/codesign", "-T", "/usr/bin/security"]),
            "security import",
        )?;
        run_capturing_output(
            std::process::Command::new("security")
                .args(["set-key-partition-list", "-S", "apple-tool:,apple:", "-k", keychain_password.as_str()])
                .arg(&keychain_path),
            "security set-key-partition-list",
        )?;
        let existing_keychains = std::process::Command::new("security")
            .args(["list-keychains", "-d", "user"])
            .output()
            .map_err(|e| format!("running security list-keychains: {e}"))?;
        let existing_keychains = String::from_utf8_lossy(&existing_keychains.stdout);
        let mut list_command = std::process::Command::new("security");
        list_command.args(["list-keychains", "-d", "user", "-s"]).arg(&keychain_path);
        for line in existing_keychains.lines() {
            let trimmed = line.trim().trim_matches('"');
            if !trimmed.is_empty() {
                list_command.arg(trimmed);
            }
        }
        run_capturing_output(&mut list_command, "security list-keychains")?;

        // `security cms -D` decodes the CMS signature wrapping a real provisioning profile --
        // there's no other supported way to read one. Extracted with a plain string search
        // rather than a full plist parser: the decoded content is themselves just XML text,
        // and only this one field is needed.
        let decoded = std::process::Command::new("security")
            .args(["cms", "-D", "-i"])
            .arg(&signing.provisioning_profile_path)
            .output()
            .map_err(|e| format!("running security cms -D: {e}"))?;
        if !decoded.status.success() {
            return Err(format!(
                "security cms -D on the provisioning profile exited with {}",
                decoded.status
            ));
        }
        let decoded_text = String::from_utf8_lossy(&decoded.stdout);
        let profile_uuid = extract_plist_string(&decoded_text, "UUID")
            .ok_or_else(|| "couldn't find a UUID in the decoded provisioning profile".to_string())?;

        let profiles_dir = dirs_home().join("Library").join("MobileDevice").join("Provisioning Profiles");
        std::fs::create_dir_all(&profiles_dir).map_err(|e| e.to_string())?;
        std::fs::copy(&signing.provisioning_profile_path, profiles_dir.join(format!("{profile_uuid}.mobileprovision")))
            .map_err(|e| e.to_string())?;

        let archive_path = staging_dir.join("RovesGame.xcarchive");
        run_capturing_output(
            std::process::Command::new("xcodebuild")
                .current_dir(staging_dir)
                .args(["-project", "RovesGame.xcodeproj", "-scheme", "RovesGame"])
                .args(["-configuration", "Release", "-sdk", "iphoneos"])
                .arg("-archivePath")
                .arg(&archive_path)
                .arg(format!("DEVELOPMENT_TEAM={}", signing.team_id))
                .arg("CODE_SIGN_STYLE=Manual")
                .arg(format!("PROVISIONING_PROFILE_SPECIFIER={profile_uuid}"))
                .arg("archive"),
            "xcodebuild archive",
        )?;

        let export_options_path = staging_dir.join("ExportOptions.plist");
        let export_options = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>app-store</string>
    <key>teamID</key>
    <string>{}</string>
    <key>signingStyle</key>
    <string>manual</string>
    <key>provisioningProfiles</key>
    <dict>
        <key>{bundle_id}</key>
        <string>{profile_uuid}</string>
    </dict>
</dict>
</plist>
"#,
            signing.team_id
        );
        std::fs::write(&export_options_path, export_options).map_err(|e| e.to_string())?;

        run_capturing_output(
            std::process::Command::new("xcodebuild")
                .arg("-exportArchive")
                .arg("-archivePath")
                .arg(&archive_path)
                .arg("-exportOptionsPlist")
                .arg(&export_options_path)
                .arg("-exportPath")
                .arg(staging_dir),
            "xcodebuild -exportArchive",
        )?;

        let matches: Vec<PathBuf> = glob::glob(&staging_dir.join("*.ipa").to_string_lossy())
            .map_err(|e| format!("invalid ipa glob pattern: {e}"))?
            .filter_map(|entry| entry.ok())
            .collect();
        let [ipa_path] = matches.as_slice() else {
            return Err(format!("expected exactly one exported .ipa, found {}", matches.len()));
        };
        Ok(ipa_path.clone())
    })();

    // The ephemeral signing keychain is always torn down, success or failure -- mirrors the
    // engine's own `_sign_and_export_ios_release`'s `finally` block.
    let _ = std::process::Command::new("security").arg("delete-keychain").arg(&keychain_path).output();

    result
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// Finds `<key>{key_name}</key>\n<string>VALUE</string>` (with arbitrary whitespace) in a
/// decoded plist's XML text and returns `VALUE` -- see `sign_and_export`'s own comment on why
/// this is a plain string search rather than a full plist parser.
fn extract_plist_string(xml: &str, key_name: &str) -> Option<String> {
    let key_marker = format!("<key>{key_name}</key>");
    let key_pos = xml.find(&key_marker)?;
    let after_key = &xml[key_pos + key_marker.len()..];
    let string_start = after_key.find("<string>")? + "<string>".len();
    let string_end = after_key[string_start..].find("</string>")?;
    Some(after_key[string_start..string_start + string_end].to_string())
}
