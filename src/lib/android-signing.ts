import { invoke } from "@tauri-apps/api/core";

// Backed by src-tauri/src/signing.rs -- see that module's own doc comment for the full
// mechanism (a locally generated/imported keystore, credentials stored in this app's own
// config directory rather than an OS keychain) and its trade-offs.
export interface AndroidSigningStatus {
  configured: boolean;
  keystorePath: string | null;
  keyAlias: string | null;
}

export async function checkAndroidSigningStatus(): Promise<AndroidSigningStatus> {
  return await invoke<AndroidSigningStatus>("check_android_signing_status");
}

/** Runs `keytool -genkeypair` with a freshly-generated random password -- see signing.rs's
 * own `generate_keystore` for the exact parameters. Downloads a portable JRE first if one
 * isn't already cached (same one android.rs's own build already uses), so this can take a
 * while on a first run. */
export async function generateAndroidKeystore(): Promise<AndroidSigningStatus> {
  return await invoke<AndroidSigningStatus>("generate_android_signing_keystore");
}

export async function importAndroidKeystore(
  keystorePath: string,
  keyAlias: string,
  storePassword: string,
  keyPassword: string,
): Promise<AndroidSigningStatus> {
  return await invoke<AndroidSigningStatus>("import_android_signing_keystore", {
    keystorePath,
    keyAlias,
    storePassword,
    keyPassword,
  });
}

/** Forgets Packmaster's own reference to the keystore (path/alias/passwords) -- does *not*
 * delete the keystore file itself, see signing.rs's own doc comment on that choice. */
export async function clearAndroidSigning(): Promise<void> {
  await invoke("clear_android_signing");
}
