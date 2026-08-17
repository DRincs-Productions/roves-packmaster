import { exists, readTextFile } from "@tauri-apps/plugin-fs";

export interface PackageJsonInfo {
  name?: string;
  version?: string;
}

/** Looks for package.json one directory up from `sourceDir` — the common bundler
 * convention (package.json lives next to the project, sourceDir is its built dist/
 * output) — mirroring src-tauri/src/packer.rs's own (now-removed) resolve_window_title
 * logic, moved here since the frontend is what owns editable name/version now.
 *
 * Builds the parent path by trimming the last segment off rather than appending `..` —
 * Tauri's fs plugin scope matching operates on the literal path string, and a literal
 * `..` segment isn't guaranteed to resolve against an allowed scope the same way a
 * plain absolute path does. */
export async function readParentPackageJson(sourceDir: string): Promise<PackageJsonInfo | null> {
  const separator = sourceDir.includes("\\") ? "\\" : "/";
  const trimmed = sourceDir.endsWith(separator) ? sourceDir.slice(0, -1) : sourceDir;
  const lastSeparator = trimmed.lastIndexOf(separator);
  if (lastSeparator <= 0) return null;
  const parentDir = trimmed.slice(0, lastSeparator);
  const path = `${parentDir}${separator}package.json`;
  if (!(await exists(path))) return null;
  try {
    const json = JSON.parse(await readTextFile(path));
    return {
      name: typeof json.name === "string" ? json.name : undefined,
      version: typeof json.version === "string" ? json.version : undefined,
    };
  } catch {
    return null;
  }
}
