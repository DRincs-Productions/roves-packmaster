import { invoke } from "@tauri-apps/api/core";

// Backed by src-tauri/src/shell.rs's cache_size/clear_cache -- every downloaded shell
// extraction lives under this app's cache dir, keyed by version/platform/variant (see that
// file's own ensure_shell), across every version ever downloaded, not just the currently
// targeted one.
export function getShellCacheSize(): Promise<number> {
  return invoke<number>("shell_cache_size");
}

export function clearShellCache(): Promise<void> {
  return invoke<void>("clear_shell_cache");
}

export function formatCacheSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unitIndex]}`;
}
