mod android;
mod bundle;
mod icon;
mod installer;
mod packer;
mod settings;
mod shell;
mod signing;

#[tauri::command]
fn check_installer_availability(platforms: Vec<String>) -> Vec<(String, bool, Option<String>)> {
    platforms
        .into_iter()
        .map(|platform| {
            let (available, reason) = installer::check_installer_availability(&platform);
            (platform, available, reason)
        })
        .collect()
}

#[tauri::command]
fn check_android_availability() -> (bool, Option<String>) {
    android::check_android_availability()
}

#[tauri::command]
fn check_android_signing_status(app: tauri::AppHandle) -> Result<signing::SigningStatus, String> {
    signing::signing_status(&app)
}

/// Downloads the same portable JRE `android.rs`'s own build already uses (cached -- a no-op
/// download if it's already there) so `keytool` (bundled with any JRE) doesn't need a
/// separate, dedicated bootstrap step of its own.
#[tauri::command]
async fn generate_android_signing_keystore(app: tauri::AppHandle) -> Result<signing::SigningStatus, String> {
    let java_home = android::ensure_jre(&app, |_| {}).await?;
    signing::generate_keystore(&app, &java_home).await
}

#[tauri::command]
fn import_android_signing_keystore(
    app: tauri::AppHandle,
    keystore_path: String,
    key_alias: String,
    store_password: String,
    key_password: String,
) -> Result<signing::SigningStatus, String> {
    signing::import_keystore(&app, keystore_path, key_alias, store_password, key_password)
}

#[tauri::command]
fn clear_android_signing(app: tauri::AppHandle) -> Result<(), String> {
    signing::clear_signing(&app)
}

#[tauri::command]
fn shell_cache_size(app: tauri::AppHandle) -> Result<u64, String> {
    shell::cache_size(&app)
}

#[tauri::command]
fn clear_shell_cache(app: tauri::AppHandle) -> Result<(), String> {
    shell::clear_cache(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            bundle::generate_release,
            bundle::check_shell_availability,
            check_installer_availability,
            check_android_availability,
            check_android_signing_status,
            generate_android_signing_keystore,
            import_android_signing_keystore,
            clear_android_signing,
            shell_cache_size,
            clear_shell_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
