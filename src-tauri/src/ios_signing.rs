//! iOS release-signing credentials for Packmaster's own iOS backend (`ios.rs`).
//!
//! The engine's own `mach bundle --ios --ios-release` (see the engine repo's own
//! CUSTOMIZATIONS.md, "Wire `--ios`/`--ios-release` into `mach bundle`" entry) reads 4
//! credentials straight from the environment
//! (`IOS_SIGNING_CERTIFICATE_P12_PATH`/`_PASSWORD`/`IOS_SIGNING_PROVISIONING_PROFILE_PATH`/
//! `IOS_SIGNING_TEAM_ID`) -- this module is Packmaster's own equivalent of `roves-action`
//! sourcing those same 4 values from GitHub Secrets: importing a real, Apple-issued
//! certificate + provisioning profile locally, and persisting enough to reuse them on the
//! next build without asking again. Same storage trade-off as `signing.rs`'s own Android
//! equivalent (plain JSON in this app's own config directory, `ios-signing.json` -- not an OS
//! keychain; see that module's own doc comment for the full reasoning, which applies
//! identically here).
//!
//! **Deliberately no `generate_*` function, unlike `signing.rs`'s `generate_keystore`.** An
//! Android keystore is self-signed by design -- Packmaster can create a complete, real one
//! locally with nothing but `keytool`. An Apple Distribution certificate must be
//! countersigned by Apple itself: it always traces back to a human with access to an
//! enrolled Apple Developer Program account (a CSR submitted through
//! developer.apple.com, a certificate + provisioning profile downloaded from there). Nothing
//! this module, Packmaster, or the engine does can produce a real one end to end -- only
//! import is offered, and the UI explains why "generate" doesn't exist here the way it does
//! for Android.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize, Deserialize)]
pub struct IosSigningConfig {
    pub certificate_p12_path: String,
    pub certificate_p12_password: String,
    pub provisioning_profile_path: String,
    pub team_id: String,
}

impl IosSigningConfig {
    /// The 4 env vars the engine's `_sign_and_export_ios_release`
    /// (`python/servo/post_build_commands.py`) reads -- setting these before invoking `mach
    /// bundle --ios --ios-release` is the entire mechanism, no further plumbing needed.
    pub fn env_vars(&self) -> [(&'static str, &str); 4] {
        [
            ("IOS_SIGNING_CERTIFICATE_P12_PATH", self.certificate_p12_path.as_str()),
            ("IOS_SIGNING_CERTIFICATE_P12_PASSWORD", self.certificate_p12_password.as_str()),
            ("IOS_SIGNING_PROVISIONING_PROFILE_PATH", self.provisioning_profile_path.as_str()),
            ("IOS_SIGNING_TEAM_ID", self.team_id.as_str()),
        ]
    }
}

/// What the frontend actually gets to see -- never the passwords, which have no reason to
/// ever leave the backend once saved.
#[derive(Clone, Serialize, Deserialize)]
pub struct IosSigningStatus {
    pub configured: bool,
    pub certificate_p12_path: Option<String>,
    pub team_id: Option<String>,
}

fn ios_signing_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("ios-signing.json"))
}

pub fn load_ios_signing_config(app: &AppHandle) -> Result<Option<IosSigningConfig>, String> {
    let path = ios_signing_config_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map(Some).map_err(|e| format!("parsing {path:?}: {e}"))
}

fn save_ios_signing_config(app: &AppHandle, config: &IosSigningConfig) -> Result<(), String> {
    let path = ios_signing_config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
}

pub fn ios_signing_status(app: &AppHandle) -> Result<IosSigningStatus, String> {
    match load_ios_signing_config(app)? {
        Some(config) => Ok(IosSigningStatus {
            configured: true,
            certificate_p12_path: Some(config.certificate_p12_path),
            team_id: Some(config.team_id),
        }),
        None => Ok(IosSigningStatus { configured: false, certificate_p12_path: None, team_id: None }),
    }
}

/// Doesn't delete the certificate/profile *files* themselves -- only forgets Packmaster's own
/// reference to them (and the password) -- same reasoning as `signing.rs`'s `clear_signing`.
pub fn clear_ios_signing(app: &AppHandle) -> Result<(), String> {
    let path = ios_signing_config_path(app)?;
    if path.is_file() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn import_ios_signing(
    app: &AppHandle,
    certificate_p12_path: String,
    certificate_p12_password: String,
    provisioning_profile_path: String,
    team_id: String,
) -> Result<IosSigningStatus, String> {
    if !Path::new(&certificate_p12_path).is_file() {
        return Err(format!("no file found at {certificate_p12_path}"));
    }
    if !Path::new(&provisioning_profile_path).is_file() {
        return Err(format!("no file found at {provisioning_profile_path}"));
    }
    if team_id.trim().is_empty() {
        return Err("Team ID can't be empty".to_string());
    }
    let config = IosSigningConfig { certificate_p12_path, certificate_p12_password, provisioning_profile_path, team_id };
    save_ios_signing_config(app, &config)?;
    ios_signing_status(app)
}
