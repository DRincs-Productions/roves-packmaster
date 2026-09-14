import { invoke } from "@tauri-apps/api/core";

// Backed by src-tauri/src/ios_signing.rs -- see that module's own doc comment for the full
// mechanism (an imported certificate + provisioning profile, credentials stored in this
// app's own config directory rather than an OS keychain) and its trade-offs.
export interface IosSigningStatus {
  configured: boolean;
  certificateP12Path: string | null;
  teamId: string | null;
}

export async function checkIosSigningStatus(): Promise<IosSigningStatus> {
  return await invoke<IosSigningStatus>("check_ios_signing_status");
}

/** Unlike Android, there is no `generateIosSigning` -- an Apple Distribution certificate must
 * be countersigned by Apple itself (a CSR submitted through your own Apple Developer Program
 * account), so only importing an already-issued certificate/profile is possible here. */
export async function importIosSigning(
  certificateP12Path: string,
  certificateP12Password: string,
  provisioningProfilePath: string,
  teamId: string,
): Promise<IosSigningStatus> {
  return await invoke<IosSigningStatus>("import_ios_signing", {
    certificateP12Path,
    certificateP12Password,
    provisioningProfilePath,
    teamId,
  });
}

/** Forgets Packmaster's own reference to the certificate/profile (paths/password/team ID) --
 * does *not* delete those files themselves, see ios_signing.rs's own doc comment on that
 * choice (mirrors clearAndroidSigning). */
export async function clearIosSigning(): Promise<void> {
  await invoke("clear_ios_signing");
}
