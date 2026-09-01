import { invoke } from "@tauri-apps/api/core";

export interface AndroidAvailability {
  available: boolean;
  reason: string | null;
}

// Real feasibility check backed by src-tauri/src/android.rs's check_android_availability --
// unlike portable desktop bundling (any host can produce any platform's zip), Android
// packaging currently only runs on Linux/macOS (see that module's own doc comment: the
// engine's Gradle build shells out to `ndk-build` with no Windows fallback).
export async function checkAndroidAvailability(): Promise<AndroidAvailability> {
  const [available, reason] = await invoke<[boolean, string | null]>("check_android_availability");
  return { available, reason };
}
