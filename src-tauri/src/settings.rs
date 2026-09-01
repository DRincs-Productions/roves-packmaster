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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettings {
    pub steam: SteamSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionSettings {
    pub enabled: bool,
    pub level: i32,
    pub max_pack_size: String,
    pub exclude: Vec<String>,
    pub boot_include: Vec<String>,
}

/// A single source icon, applied everywhere it's possible to apply it -- see `bundle.rs`'s
/// own `apply_icon` and `icon.rs` for what "everywhere possible" means per platform, and why
/// it used to be two separate settings (a PNG for the runtime window icon, a pre-made `.ico`
/// for the packaged `.exe`'s own icon resource) instead of one.
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileSettings {
    pub android: MobilePlatformSettings,
    pub advanced: MobileAdvancedSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackmasterSettings {
    pub source_dir: Option<String>,
    pub portable: PortableSettings,
    pub installers: InstallerSettings,
    pub mobile: MobileSettings,
    pub plugins: PluginSettings,
    pub compression: CompressionSettings,
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

    /// `(platform_id, enabled)` pairs, in the same order release.yml's own matrix builds
    /// them — used to iterate only the platforms the user actually asked for.
    pub fn selected(&self) -> Vec<&'static str> {
        [("windows", self.windows), ("linux", self.linux), ("macos", self.macos)]
            .into_iter()
            .filter_map(|(platform, enabled)| enabled.then_some(platform))
            .collect()
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
