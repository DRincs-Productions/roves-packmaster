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
  /** Use the engine's Steam player packages (`roves_shell_<platform>_steam.zip`). */
  enabled: boolean;
  /** Steam App ID, digits only. Written into a `steam_appid.txt` next to the packaged
   * game's executable when `enabled` (Valve's own convention for local testing outside the
   * Steam client) -- see src-tauri/src/cef_player.rs. */
  appId: string;
}

/** The players' local MCP bridge (src-tauri/src/cef_player.rs): every desktop output
 * (Windows, macOS, Linux) uses the engine's `roves_shell_<platform>_mcp.zip` development
 * package. Off by default; can't be combined with Steam (the engine publishes no such
 * package), so configure.tsx turns it off when Steam is turned on. */
export interface McpSettings {
  enabled: boolean;
}

export interface PluginSettings {
  steam: SteamSettings;
  mcp: McpSettings;
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
  /** Build a real, signed release .apk instead of the default debug one -- see
   * lib/android-signing.ts for the keystore this relies on. A separate opt-in from "is a
   * keystore configured at all": generating/importing one doesn't itself switch future
   * builds to release mode. */
  releaseSigningEnabled: boolean;
}

/** iOS's own equivalent of `MobileAdvancedSettings` -- a separate interface rather than
 * reusing that one, since the two platforms' override fields genuinely differ (`bundleId`
 * instead of `orientation`; iOS has no equivalent to Android's screen-orientation lock).
 * Mirrors the engine's own `--ios-app-name`/`--ios-bundle-id` `mach bundle` flags. */
export interface IosAdvancedSettings {
  appName: string;
  bundleId: string;
  /** Archive and export a real, signed .ipa instead of the default unsigned Simulator build
   * -- see lib/ios-signing.ts for the certificate/provisioning profile this relies on. Same
   * separate-opt-in reasoning as Android's `releaseSigningEnabled`. */
  releaseSigningEnabled: boolean;
}

export interface MobileSettings {
  android: MobilePlatformSettings;
  advanced: MobileAdvancedSettings;
  ios: MobilePlatformSettings;
  iosAdvanced: IosAdvancedSettings;
}

/** The Release info icon (a PNG), the first source of the project icon. It becomes the
 * window icon of every desktop player, and a `.ico`/`.icns` derived from it the Windows
 * `play.exe` icon and the macOS bundle icon -- see src-tauri/src/cef_player.rs's
 * `resolve_project_icon` for the fallbacks (the build's own icon.png/icon.ico/favicon.ico,
 * then the Roves icon). `null` means use those fallbacks. Android uses it too. */
export interface IconSettings {
  path: string | null;
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
  icon: IconSettings;
}

// Defaults: every portable desktop player, nothing else (no installer, no mobile, no Steam,
// no MCP); the project icon comes from the build itself when none is picked.
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
    advanced: { appName: "", orientation: "", releaseSigningEnabled: false },
    ios: { enabled: false },
    iosAdvanced: { appName: "", bundleId: "", releaseSigningEnabled: false },
  },
  plugins: {
    // 480 is Valve's own well-known Steamworks test App ID (Spacewar) — a sensible default
    // to try things with, rather than an empty field that always needs typing into first.
    steam: { enabled: false, appId: "480" },
    mcp: { enabled: false },
  },
  icon: {
    path: null,
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
  // setting doesn't crash on an older, incomplete stored settings file. Settings removed
  // since (the Servo shell's `compression`, the manual CEF base folder `cefWindows`) are
  // dropped instead of being carried along forever.
  const {
    compression: _compression,
    cefWindows: _cefWindows,
    ...current
  } = stored as PackmasterSettings & { compression?: unknown; cefWindows?: unknown };
  return {
    ...defaultSettings,
    ...current,
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
      ios: { ...defaultSettings.mobile.ios, ...stored.mobile?.ios },
      iosAdvanced: { ...defaultSettings.mobile.iosAdvanced, ...stored.mobile?.iosAdvanced },
    },
    plugins: {
      steam: { ...defaultSettings.plugins.steam, ...stored.plugins?.steam },
      mcp: { ...defaultSettings.plugins.mcp, ...stored.plugins?.mcp },
    },
    icon: { ...defaultSettings.icon, ...stored.icon },
  };
}

export async function saveSettings(settings: PackmasterSettings): Promise<void> {
  const store = getStore();
  await store.set(SETTINGS_KEY, settings);
  await store.save();
}
