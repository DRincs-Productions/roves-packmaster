import { invoke } from "@tauri-apps/api/core";

// Real feasibility check ("is this actually distributable") backed by
// src-tauri/src/bundle.rs's `check_shell_availability` — a live HEAD request against the
// targeted shell release's actual download URL per platform, not an assumption.
export async function checkShellAvailability(
  platforms: string[],
): Promise<Record<string, boolean>> {
  const pairs = await invoke<[string, boolean][]>("check_shell_availability", { platforms });
  return Object.fromEntries(pairs);
}
