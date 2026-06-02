// Operon GUI - Build Script
//
// This build script is required by Tauri to generate necessary context
// and configuration at compile time. It processes the tauri.conf.json
// file and generates Rust code that's used by the tauri::generate_context!()
// macro in main.rs.

fn main() {
    tauri_build::build()
}
