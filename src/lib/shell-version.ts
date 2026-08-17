// The Roves engine shell version this build of Packmaster currently targets — see the
// engine repo's own CLAUDE.md ("Cutting a versioned release" section), which requires
// bumping this constant every time a new shell version is published.
export const TARGET_SHELL_VERSION = "v0.1.1";

const LATEST_RELEASE_API_URL =
  "https://api.github.com/repos/DRincs-Productions/roves/releases/latest";

export const ROVES_RELEASES_URL = "https://github.com/DRincs-Productions/roves/releases/latest";

export interface ShellVersionCheckResult {
  current: string;
  latest: string;
  isUpdateAvailable: boolean;
}

function parseVersion(tag: string): [number, number, number] | null {
  const match = /^v?(\d+)\.(\d+)\.(\d+)/.exec(tag);
  if (!match) return null;
  return [Number(match[1]), Number(match[2]), Number(match[3])];
}

function isNewer(latest: string, current: string): boolean {
  const latestParts = parseVersion(latest);
  const currentParts = parseVersion(current);
  if (!latestParts || !currentParts) return false;
  for (let i = 0; i < 3; i++) {
    if (latestParts[i] !== currentParts[i]) return latestParts[i] > currentParts[i];
  }
  return false;
}

// Fails silently (returns null) on any network/parse error — this is a non-critical,
// best-effort notice, not something that should ever block or error out Packmaster itself.
export async function checkForNewShellVersion(): Promise<ShellVersionCheckResult | null> {
  try {
    const response = await fetch(LATEST_RELEASE_API_URL);
    if (!response.ok) return null;
    const data = await response.json();
    const latest = typeof data?.tag_name === "string" ? data.tag_name : null;
    if (!latest) return null;
    return {
      current: TARGET_SHELL_VERSION,
      latest,
      isUpdateAvailable: isNewer(latest, TARGET_SHELL_VERSION),
    };
  } catch {
    return null;
  }
}
