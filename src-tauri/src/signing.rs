//! Android release-signing credentials for Packmaster's own Android backend (`android.rs`).
//!
//! The engine's own `mach bundle --android --android-release` (see the engine repo's own
//! CUSTOMIZATIONS.md, "`mach bundle --android-release`" entry) reads 4 credentials straight
//! from the environment (`APK_SIGNING_KEY_STORE_PATH`/`_STORE_PASS`/`_ALIAS`/`_PASS` --
//! `support/android/apk/buildSrc/src/main/kotlin/Android.kt`'s `getSigningKeyInfo`, upstream
//! Servo's own, unpatched) -- this module is Packmaster's own equivalent of `roves-action`
//! sourcing those same 4 values from a GitHub Secret: generating or importing a real keystore
//! locally, and persisting enough to reuse it on the next build without asking again.
//!
//! **Credential storage is a deliberate, documented trade-off, not an oversight**: these are
//! saved as plain JSON in this app's own config directory
//! (`android-signing.json`, *not* `settings.json` -- this is machine-local secret material,
//! never meant to be shared/committed the way a project's bundling settings might be),
//! relying on the OS's own per-user file permissions rather than an OS keychain
//! (Credential Manager/Keychain/Secret Service). Same trust model as `~/.android/
//! debug.keystore` itself, or countless other local dev tools (`~/.netrc`, `~/.aws/
//! credentials`, ...) -- not encrypted at rest, but never transmitted anywhere either. A
//! real OS-keychain integration (e.g. the `keyring` crate) would be a strict improvement,
//! deliberately not attempted here: it pulls in per-platform native credential-store
//! dependencies (and, on Linux, a Secret Service daemon that may not even be running,
//! particularly in a CI-like or headless context) this module has no way to test in this
//! session -- see this repo's own TODO.md for tracking that as a follow-up rather than
//! guessing at correctness blind.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::android::script_command;

#[derive(Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    pub keystore_path: String,
    pub key_alias: String,
    pub store_password: String,
    pub key_password: String,
}

impl SigningConfig {
    /// The 4 env vars `getSigningKeyInfo` (see this module's own doc comment) reads --
    /// setting these on the `gradlew` `Command` before invoking it is the entire mechanism,
    /// no further plumbing needed on the Gradle/Kotlin side at all.
    pub fn env_vars(&self) -> [(&'static str, &str); 4] {
        [
            ("APK_SIGNING_KEY_STORE_PATH", self.keystore_path.as_str()),
            ("APK_SIGNING_KEY_STORE_PASS", self.store_password.as_str()),
            ("APK_SIGNING_KEY_ALIAS", self.key_alias.as_str()),
            ("APK_SIGNING_KEY_PASS", self.key_password.as_str()),
        ]
    }
}

/// What the frontend actually gets to see -- never the passwords, which have no reason to
/// ever leave the backend once saved.
#[derive(Clone, Serialize, Deserialize)]
pub struct SigningStatus {
    pub configured: bool,
    pub keystore_path: Option<String>,
    pub key_alias: Option<String>,
}

fn signing_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("android-signing.json"))
}

pub fn load_signing_config(app: &AppHandle) -> Result<Option<SigningConfig>, String> {
    let path = signing_config_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map(Some).map_err(|e| format!("parsing {path:?}: {e}"))
}

fn save_signing_config(app: &AppHandle, config: &SigningConfig) -> Result<(), String> {
    let path = signing_config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
}

pub fn signing_status(app: &AppHandle) -> Result<SigningStatus, String> {
    match load_signing_config(app)? {
        Some(config) => Ok(SigningStatus {
            configured: true,
            keystore_path: Some(config.keystore_path),
            key_alias: Some(config.key_alias),
        }),
        None => Ok(SigningStatus { configured: false, keystore_path: None, key_alias: None }),
    }
}

/// Doesn't delete the keystore *file* itself -- only forgets Packmaster's own reference to
/// it (and the passwords) -- deleting someone's actual private key material without being
/// asked to, specifically, is a much worse mistake than leaving an unused file on disk.
pub fn clear_signing(app: &AppHandle) -> Result<(), String> {
    let path = signing_config_path(app)?;
    if path.is_file() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn import_keystore(
    app: &AppHandle,
    keystore_path: String,
    key_alias: String,
    store_password: String,
    key_password: String,
) -> Result<SigningStatus, String> {
    if !Path::new(&keystore_path).is_file() {
        return Err(format!("no file found at {keystore_path}"));
    }
    let config = SigningConfig { keystore_path, key_alias, store_password, key_password };
    save_signing_config(app, &config)?;
    signing_status(app)
}

/// A cryptographically-irrelevant length limit (`keytool` doesn't need a "strong" password
/// chosen by a human to remember it -- this is generated once and stored, never typed) --
/// just long enough that brute-forcing it isn't the weak point in this whole scheme (the
/// plaintext-on-disk storage this module's own doc comment already discloses is).
fn random_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
        ^ (std::process::id() as u128);
    let mut out = String::with_capacity(32);
    for _ in 0..32 {
        // A plain linear congruential generator -- not cryptographically secure, but this
        // only needs to be unguessable-in-practice for a locally-stored, never-transmitted
        // password, not withstand a dedicated adversary; pulling in a real CSPRNG crate for
        // this one call isn't worth the extra dependency.
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let index = ((seed >> 64) as usize) % CHARS.len();
        out.push(CHARS[index] as char);
    }
    out
}

/// Generates a real, self-signed keystore via `keytool` (bundled with the same portable JRE
/// `android.rs`'s own `ensure_jre` downloads -- no separate JDK/Android Studio install
/// needed, same principle as everything else this app bootstraps itself). 10000-day validity
/// (~27 years) and a generic distinguished name -- matches what `keytool`'s own interactive
/// defaults would produce for someone who doesn't care about the certificate's identity
/// fields, only that Android accepts it as a release signature.
pub async fn generate_keystore(app: &AppHandle, java_home: &Path) -> Result<SigningStatus, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let keystore_path = dir.join("android-release.keystore");

    let store_password = random_password();
    let key_password = store_password.clone();
    let key_alias = "packmaster".to_string();

    let keytool = java_home.join("bin").join(if cfg!(windows) { "keytool.exe" } else { "keytool" });
    let status = script_command(&keytool)
        .arg("-genkeypair")
        .arg("-v")
        .arg("-keystore")
        .arg(&keystore_path)
        .arg("-alias")
        .arg(&key_alias)
        .arg("-keyalg")
        .arg("RSA")
        .arg("-keysize")
        .arg("2048")
        .arg("-validity")
        .arg("10000")
        .arg("-storepass")
        .arg(&store_password)
        .arg("-keypass")
        .arg(&key_password)
        .arg("-dname")
        .arg("CN=Roves Packmaster, OU=Packmaster, O=Packmaster, L=Unknown, ST=Unknown, C=US")
        .status()
        .map_err(|e| format!("running keytool: {e}"))?;
    if !status.success() {
        return Err(format!("keytool exited with {status}"));
    }

    let config = SigningConfig {
        keystore_path: keystore_path.to_string_lossy().into_owned(),
        key_alias,
        store_password,
        key_password,
    };
    save_signing_config(app, &config)?;
    signing_status(app)
}
