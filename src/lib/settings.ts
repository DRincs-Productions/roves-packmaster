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

export interface SteamSettings {
  enabled: boolean;
  /** Steam App ID, digits only. Written into a `steam_appid.txt` next to the packaged
   * game's executable when `enabled` (Valve's own convention for local testing outside the
   * Steam client) -- see bundle.rs's `generate_release`. */
  appId: string;
}

export interface PluginSettings {
  steam: SteamSettings;
}

export interface CompressionSettings {
  enabled: boolean;
  level: number;
  maxPackSize: string;
  exclude: string[];
  bootInclude: string[];
}

export type MobileOrientation =
  | "any"
  | "natural"
  | "landscape"
  | "landscape-primary"
  | "landscape-secondary"
  | "portrait"
  | "portrait-primary"
  | "portrait-secondary";

/** Just the "is this platform included in the release" toggle -- see MobileAdvancedSettings
 * for the settings shared across every mobile platform. */
export interface MobilePlatformSettings {
  enabled: boolean;
}

/** Shared across every mobile platform (Android today, iOS later) rather than duplicated per
 * platform -- a game's app name and screen orientation don't differ by mobile platform.
 * Mirrors the engine's own `--android-app-name`/`--android-orientation` `mach bundle` flags.
 * Manual values here are only actually used when configure.tsx's "use info from your web app
 * manifest" switch is off, or for a field the manifest itself doesn't set -- that switch isn't
 * persisted here (see configure.tsx's own comment): whether a project has a web app manifest
 * is a per-project fact, not a global preference, so it's re-derived every time the source
 * folder changes instead of being remembered across unrelated projects.
 *
 * Deliberately no status-bar-color/theme-color setting: a game is expected to run edge-to-
 * edge with no visible status bar at all (a player can still pull it down like on any other
 * Android app), so there's nothing to theme -- this isn't a configurable option. */
export interface MobileAdvancedSettings {
  appName: string;
  orientation: MobileOrientation | "";
}

export interface MobileSettings {
  android: MobilePlatformSettings;
  advanced: MobileAdvancedSettings;
}

export interface IconSettings {
  /** Window/taskbar icon (PNG), copied next to the packaged binary -- see bundle.rs's
   * `apply_icon`. Not supported on macOS yet (its own Dock/app icon has no runtime override
   * -- see the engine repo's own CUSTOMIZATIONS.md). */
  pngPath: string | null;
  /** Windows-only: the packaged play.exe's own icon resource, patched in place via rcedit
   * after bundling -- see bundle.rs's `apply_icon`. Ignored on other platforms. */
  icoPath: string | null;
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
  mobile: MobileSettings;
  plugins: PluginSettings;
  compression: CompressionSettings;
  icon: IconSettings;
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
  mobile: {
    android: { enabled: false },
    advanced: { appName: "", orientation: "" },
  },
  plugins: {
    // 480 is Valve's own well-known Steamworks test App ID (Spacewar) — a sensible default
    // to try things with, rather than an empty field that always needs typing into first.
    steam: { enabled: false, appId: "480" },
  },
  compression: {
    enabled: true, // --content-compress=auto is the engine's own default
    level: 1,
    maxPackSize: "500M",
    exclude: [],
    bootInclude: [],
  },
  icon: {
    pngPath: null,
    icoPath: null,
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
    mobile: {
      android: { ...defaultSettings.mobile.android, ...stored.mobile?.android },
      advanced: { ...defaultSettings.mobile.advanced, ...stored.mobile?.advanced },
    },
    plugins: { steam: { ...defaultSettings.plugins.steam, ...stored.plugins?.steam } },
    compression: { ...defaultSettings.compression, ...stored.compression },
    icon: { ...defaultSettings.icon, ...stored.icon },
  };
}

export async function saveSettings(settings: PackmasterSettings): Promise<void> {
  const store = getStore();
  await store.set(SETTINGS_KEY, settings);
  await store.save();
}
