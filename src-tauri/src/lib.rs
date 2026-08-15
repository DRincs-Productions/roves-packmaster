// `@tauri-apps/api/path`'s own `executableDir()` is explicitly "Not supported"
// on Windows and macOS (only resolves on Linux, to an XDG bin convention) —
// nowhere near "the directory the currently running executable lives in",
// which is what releases need to be written next to. `std::env::current_exe`
// is the reliable, cross-platform way to get that, so it's exposed here
// instead of trying to fake it from the JS side.
#[tauri::command]
fn get_executable_dir() -> Result<String, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())
        .map(|p| p.to_string_lossy().into_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![get_executable_dir])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
