import { invoke } from "@tauri-apps/api/core";

export interface InstallerAvailability {
  available: boolean;
  reason: string | null;
}

// Real feasibility check backed by src-tauri/src/installer.rs's check_installer_availability
// — unlike portable bundling (any host can produce any platform's zip, since it's just a
// prebuilt download), an installer can only be built when Packmaster itself is running on
// that exact platform *and* the native tool it needs (WiX/hdiutil/dpkg-deb) is installed.
export async function checkInstallerAvailability(
  platforms: string[],
): Promise<Record<string, InstallerAvailability>> {
  const triples = await invoke<[string, boolean, string | null][]>("check_installer_availability", {
    platforms,
  });
  return Object.fromEntries(
    triples.map(([platform, available, reason]) => [platform, { available, reason }]),
  );
}
