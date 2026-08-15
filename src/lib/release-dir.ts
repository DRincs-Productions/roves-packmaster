import { invoke } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";

/** The folder generated releases are written to — always next to wherever
 * Packmaster itself is currently running from (asked for directly), which
 * needs a real OS-level executable path, not anything `@tauri-apps/api/path`
 * exposes on its own (see `src-tauri/src/lib.rs`'s `get_executable_dir`). */
export async function getReleaseDir(): Promise<string> {
  const exeDir = await invoke<string>("get_executable_dir");
  return join(exeDir, "release");
}
