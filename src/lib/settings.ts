import { LazyStore } from "@tauri-apps/plugin-store";

export type InstallerFormat = "msi" | "deb" | "dmg";

export interface PortableSettings {
  windows: boolean;
  linux: boolean;
  macos: boolean;
}

export interface InstallerPlatformSettings {
  enabled: boolean;
  /** Multi-select, even though each platform only has one real format today (see
   * README.md's "nsis/rpm/appimage aren't implemented yet") — a select-one-of-many field
   * would need reshaping the moment a second format per platform exists, a multi-select
   * doesn't. */
  formats: string[];
}

export interface InstallerSettings {
  windows: InstallerPlatformSettings;
  linux: InstallerPlatformSettings;
  macos: InstallerPlatformSettings;
}

export interface PluginSettings {
  steam: boolean;
}

export interface CompressionSettings {
  enabled: boolean;
  level: number;
  maxPackSize: string;
  exclude: string[];
  bootInclude: string[];
}

export interface ReleaseInfo {
  name: string;
  version: string;
}

export interface PackmasterSettings {
  sourceDir: string | null;
  /** Remembered name/version per source folder — see configure.tsx's own release-info
   * effect for how these get derived from package.json vs. remembered as-is. Keyed by the
   * exact sourceDir path, so picking a different folder starts fresh rather than carrying
   * over an unrelated game's name/version. */
  releaseInfoByPath: Record<string, ReleaseInfo>;
  portable: PortableSettings;
  installers: InstallerSettings;
  plugins: PluginSettings;
  compression: CompressionSettings;
}

// Mirrors `mach bundle`'s own defaults (see the engine's
// python/servo/post_build_commands.py) so a user who never touches these
// screens still gets exactly what a plain `mach bundle` invocation would
// have produced.
export const defaultSettings: PackmasterSettings = {
  sourceDir: null,
  releaseInfoByPath: {},
  portable: {
    windows: true,
    linux: true,
    macos: true,
  },
  installers: {
    windows: { enabled: false, formats: [] },
    linux: { enabled: false, formats: [] },
    macos: { enabled: false, formats: [] },
  },
  plugins: {
    steam: false,
  },
  compression: {
    enabled: true, // --content-compress=auto is the engine's own default
    level: 1,
    maxPackSize: "500M",
    exclude: [],
    bootInclude: [],
  },
};

// A single persisted store, loaded lazily on first access — every user
// change to any setting on the configure screen is written back here, and
// read back the next time Packmaster starts (see this project's own
// CLAUDE.md and the main engine repo's CLAUDE.md, which requires asking
// whether any *new* shell setting should also get a home here).
const STORE_FILE = "settings.json";
const SETTINGS_KEY = "settings";

let storeInstance: LazyStore | null = null;

function getStore(): LazyStore {
  if (!storeInstance) {
    storeInstance = new LazyStore(STORE_FILE);
  }
  return storeInstance;
}

export async function loadSettings(): Promise<PackmasterSettings> {
  const store = getStore();
  const stored = await store.get<PackmasterSettings>(SETTINGS_KEY);
  if (!stored) {
    return defaultSettings;
  }
  // Shallow-merged with defaults so a Packmaster upgrade that adds a new
  // setting doesn't crash on an older, incomplete stored settings file.
  return {
    ...defaultSettings,
    ...stored,
    releaseInfoByPath: { ...defaultSettings.releaseInfoByPath, ...stored.releaseInfoByPath },
    portable: { ...defaultSettings.portable, ...stored.portable },
    installers: {
      windows: { ...defaultSettings.installers.windows, ...stored.installers?.windows },
      linux: { ...defaultSettings.installers.linux, ...stored.installers?.linux },
      macos: { ...defaultSettings.installers.macos, ...stored.installers?.macos },
    },
    plugins: { ...defaultSettings.plugins, ...stored.plugins },
    compression: { ...defaultSettings.compression, ...stored.compression },
  };
}

export async function saveSettings(settings: PackmasterSettings): Promise<void> {
  const store = getStore();
  await store.set(SETTINGS_KEY, settings);
  await store.save();
}
