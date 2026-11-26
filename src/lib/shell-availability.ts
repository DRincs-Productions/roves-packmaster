import { invoke } from "@tauri-apps/api/core";

// Real feasibility check ("is this actually distributable") backed by
// src-tauri/src/bundle.rs's `check_shell_availability` — a live HEAD request against the
// targeted engine release's player package URL per platform, not an assumption. `mcp` picks
// the `_mcp` development package of every platform.
export async function checkShellAvailability(
  platforms: string[],
  steam: boolean,
  mcp: boolean,
): Promise<Record<string, boolean>> {
  const pairs = await invoke<[string, boolean][]>("check_shell_availability", {
    platforms,
    steam,
    mcp,
  });
  return Object.fromEntries(pairs);
}
