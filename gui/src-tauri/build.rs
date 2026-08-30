use std::path::Path;
use std::process::Command;

fn main() {
    // Re-run this build script if TypeScript source files change
    println!("cargo:rerun-if-changed=../src/ts");
    println!("cargo:rerun-if-changed=../package.json");
    println!("cargo:rerun-if-changed=../tsconfig.json");

    let js_entry = Path::new("../src/js/main.js");
    let needs_build = !js_entry.exists();

    if needs_build {
        // Hey friend! If the repository was freshly cloned and `gui/src/js` is missing,
        // we automatically run `npm run build` so the embedded web assets are properly compiled!
        let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };
        let status = Command::new(npm_cmd)
            .args(["run", "build"])
            .current_dir("..")
            .status();

        if let Err(err) = status {
            eprintln!("cargo:warning=Failed to run npm run build in gui: {err}. Please run 'npm install && npm run build' in gui/ directory.");
        }
    }

    tauri_build::build();
}
