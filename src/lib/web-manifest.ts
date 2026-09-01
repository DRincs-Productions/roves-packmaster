import { exists, readTextFile } from "@tauri-apps/plugin-fs";

export interface WebManifestInfo {
  file: string;
  name?: string;
  shortName?: string;
  orientation?: string;
  themeColor?: string;
}

// Same 3 filenames, same order, as the engine's own `_read_web_manifest` (see
// python/servo/post_build_commands.py in the main roves repo) -- kept in sync so
// Packmaster's "use info from your web app manifest" default matches exactly what
// `mach bundle --android` itself would pick up.
const CANDIDATES = ["manifest.webmanifest", "manifest.json", "site.webmanifest"];

/** Looks for the game's own web app manifest directly inside `sourceDir` (unlike
 * readParentPackageJson, which looks one directory up -- a web app manifest ships inside the
 * built output itself, the same place `--content-dir` points `mach bundle` at). Returns null
 * if none of the 3 candidates exist or parse as a JSON object. */
export async function readWebManifest(sourceDir: string): Promise<WebManifestInfo | null> {
  const separator = sourceDir.includes("\\") ? "\\" : "/";
  const base = sourceDir.endsWith(separator) ? sourceDir : `${sourceDir}${separator}`;
  for (const candidate of CANDIDATES) {
    const path = `${base}${candidate}`;
    if (!(await exists(path))) continue;
    try {
      // biome-ignore lint/suspicious/noExplicitAny: parsing an arbitrary external JSON file
      const json: any = JSON.parse(await readTextFile(path));
      if (typeof json !== "object" || json === null) continue;
      return {
        file: candidate,
        name: typeof json.name === "string" ? json.name : undefined,
        shortName: typeof json.short_name === "string" ? json.short_name : undefined,
        orientation: typeof json.orientation === "string" ? json.orientation : undefined,
        themeColor: typeof json.theme_color === "string" ? json.theme_color : undefined,
      };
    } catch {
      // Malformed JSON -- try the next candidate rather than failing outright.
    }
  }
  return null;
}
