//! Mirrors `src/lib/settings.ts`'s `PackmasterSettings` on the Rust side, for deserializing
//! what the frontend sends into `generate_release`.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableSettings {
    pub windows: bool,
    pub linux: bool,
    pub macos: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerPlatformSettings {
    pub enabled: bool,
    pub formats: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerSettings {
    pub windows: InstallerPlatformSettings,
    pub linux: InstallerPlatformSettings,
    pub macos: InstallerPlatformSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamSettings {
    pub enabled: bool,
    pub app_id: String,
}

/// The players' local MCP bridge (roves-cli, `roves_cli.rs`): every desktop output uses the engine's
/// `roves_shell_<platform>_mcp.zip` asset. Off by default; can't be combined with Steam (no
/// such engine package, `shell::asset_name`). Absent in older stored settings.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpSettings {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettings {
    pub steam: SteamSettings,
    #[serde(default)]
    pub mcp: McpSettings,
}

/// The Release info icon (a PNG), the first source of the project icon -- see
/// roves-cli's project icon rules (`--icon-png`) for the per-platform precedence and fallbacks.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconSettings {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobilePlatformSettings {
    pub enabled: bool,
}

/// Shared across every mobile platform (Android today) — mirrors `src/lib/settings.ts`'s
/// `MobileAdvancedSettings`. Both fields are manual *overrides*: an empty string means
/// "nothing explicitly set", in which case `android.rs`'s own `read_web_manifest` resolves
/// the real value from the project's web app manifest instead — the frontend's own "use info
/// from your web app manifest" switch is deliberately not sent here at all (see
/// `configure.tsx`'s own comment: it's per-project derived UI state, not a setting), so this
/// same "non-empty override always wins, else fall back to the manifest" precedence is the
/// only thing that needs to agree between the two sides.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileAdvancedSettings {
    pub app_name: String,
    pub orientation: String,
    /// Build a real, signed release `.apk` (via `signing.rs`'s locally-generated/imported
    /// keystore) instead of the default debug one. A separate, explicit opt-in rather than
    /// "sign automatically whenever a keystore happens to be configured" -- a debug build
    /// stays the quick, no-keystore-needed default even for someone who's already set one
    /// up, e.g. while iterating and not yet ready to hand out a release build.
    pub release_signing_enabled: bool,
}

/// iOS's own equivalent of `MobileAdvancedSettings` -- a separate struct rather than reusing
/// that one, since the two platforms' override fields genuinely differ (`bundle_id` instead
/// of `orientation`; iOS has no equivalent to Android's screen-orientation lock). Mirrors
/// `src/lib/settings.ts`'s `IosAdvancedSettings`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IosAdvancedSettings {
    pub app_name: String,
    pub bundle_id: String,
    /// Archive and export a real, signed `.ipa` (via `ios_signing.rs`'s imported certificate
    /// and provisioning profile) instead of the default unsigned Simulator build. Same explicit
    /// opt-in reasoning as Android's `release_signing_enabled` above.
    pub release_signing_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileSettings {
    pub android: MobilePlatformSettings,
    pub advanced: MobileAdvancedSettings,
    pub ios: MobilePlatformSettings,
    pub ios_advanced: IosAdvancedSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackmasterSettings {
    pub source_dir: Option<String>,
    pub portable: PortableSettings,
    pub installers: InstallerSettings,
    pub mobile: MobileSettings,
    pub plugins: PluginSettings,
    pub icon: IconSettings,
}

impl PortableSettings {
    pub fn get(&self, platform: &str) -> bool {
        match platform {
            "windows" => self.windows,
            "linux" => self.linux,
            "macos" => self.macos,
            _ => false,
        }
    }
}

impl InstallerSettings {
    pub fn get(&self, platform: &str) -> &InstallerPlatformSettings {
        match platform {
            "windows" => &self.windows,
            "linux" => &self.linux,
            _ => &self.macos,
        }
    }
}
