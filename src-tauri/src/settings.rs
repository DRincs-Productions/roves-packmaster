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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackmasterSettings {
    pub source_dir: Option<String>,
    pub portable: PortableSettings,
    pub installers: InstallerSettings,
    pub plugins: PluginSettings,
    pub compression: CompressionSettings,
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
