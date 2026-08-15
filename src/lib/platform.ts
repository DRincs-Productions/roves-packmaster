import { platform } from "@tauri-apps/plugin-os";

export type HostPlatform = "windows" | "macos" | "linux";

let cached: HostPlatform | null = null;

/**
 * Which installer format(s) are actually buildable is tied to a real,
 * host-specific tool — WiX's candle/light for `.msi` (Windows only),
 * `hdiutil` for `.dmg` (macOS only), `dpkg-deb` for `.deb` (Linux only,
 * see the engine's own `python/servo/post_build_commands.py`). Portable
 * bundling has no such constraint (see `settings.ts`'s defaults) since it
 * only needs a pre-built engine binary, not a host packaging tool — so
 * only the installer section hides options based on this.
 */
export async function getHostPlatform(): Promise<HostPlatform> {
  if (cached) return cached;
  const raw = await platform();
  cached = raw === "macos" ? "macos" : raw === "linux" ? "linux" : "windows";
  return cached;
}
