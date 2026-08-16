mod bundle;
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
            check_installer_availability
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
