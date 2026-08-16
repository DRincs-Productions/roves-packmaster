//! Mirrors `src/lib/settings.ts`'s `PackmasterSettings` on the Rust side, for deserializing
//! what the frontend sends into `generate_release`. Only `sourceDir`, `portable`, and
//! `compression` currently drive real behavior (see `bundle.rs`) — `installers` and
//! `plugins.steam` are accepted (so the whole settings object round-trips without the
//! frontend needing a stripped-down variant) but not yet acted on: both need something this
//! first real-integration pass deliberately doesn't have (native per-platform installer
//! tooling, and a Steam-enabled prebuilt shell variant — see the engine repo's CLAUDE.md).

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
    pub compression: CompressionSettings,
}

impl PortableSettings {
    /// `(platform_id, enabled)` pairs, in the same order release.yml's own matrix builds
    /// them — used to iterate only the platforms the user actually asked for.
    pub fn selected(&self) -> Vec<&'static str> {
        [("windows", self.windows), ("linux", self.linux), ("macos", self.macos)]
            .into_iter()
            .filter_map(|(platform, enabled)| enabled.then_some(platform))
            .collect()
    }
}
