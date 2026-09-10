import { invoke } from "@tauri-apps/api/core";

export interface AndroidAvailability {
  available: boolean;
  reason: string | null;
}

// Real feasibility check backed by src-tauri/src/android.rs's check_android_availability --
// always available as of 2026-09-10 (see that module's own doc comment for the Windows
// history: it used to be Linux/macOS-only until an engine-side Gradle fix landed, unverified
// against a real Windows build).
export async function checkAndroidAvailability(): Promise<AndroidAvailability> {
  const [available, reason] = await invoke<[boolean, string | null]>("check_android_availability");
  return { available, reason };
}
