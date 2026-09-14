import { invoke } from "@tauri-apps/api/core";

export interface IosAvailability {
  available: boolean;
  reason: string | null;
}

// Real feasibility check backed by src-tauri/src/ios.rs's check_ios_availability -- unlike
// Android, this is a genuine, permanent restriction: only true when Packmaster itself is
// running on macOS with Xcode + XcodeGen installed (Apple-only tools, no cross-platform
// equivalent exists).
export async function checkIosAvailability(): Promise<IosAvailability> {
  const [available, reason] = await invoke<[boolean, string | null]>("check_ios_availability");
  return { available, reason };
}
