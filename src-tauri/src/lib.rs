mod android;
mod bundle;
mod icon;
mod installer;
mod packer;
mod settings;
mod shell;

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
            shell_cache_size,
            clear_shell_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
