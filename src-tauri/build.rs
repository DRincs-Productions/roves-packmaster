fn main() {
    // Rebuild if this changes -- shell.rs's `is_test_build()` reads it via `option_env!` at
    // compile time, and Cargo doesn't otherwise track env vars as build-fingerprint inputs
    // (harmless in CI, which always builds from a clean checkout anyway, but without this a
    // local `cargo build`/`tauri dev` run wouldn't notice toggling it without a full clean).
    println!("cargo:rerun-if-env-changed=PACKMASTER_TEST_BUILD");
    tauri_build::build()
}
