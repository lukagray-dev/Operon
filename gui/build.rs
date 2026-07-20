//! Build script for the GUI crate.
//!
//! Slint compiles the `.slint` files into Rust code at build time, so the build
//! script needs to do two things:
//! - tell Cargo which files should trigger a rebuild
//! - ask `slint-build` to compile the top-level window markup

use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Watch the UI and asset trees recursively so edits to any imported file
    // trigger a rebuild. That keeps the titlebar and the icons in sync with the
    // generated Rust code.
    watch_tree(&manifest_dir.join("ui"));
    watch_tree(&manifest_dir.join("assets"));

    slint_build::compile("ui/window.slint").expect("failed to compile the Operon Slint UI");
}

fn watch_tree(path: &Path) {
    if path.is_dir() {
        println!("cargo:rerun-if-changed={}", path.display());

        let entries = fs::read_dir(path).unwrap_or_else(|error| {
            panic!("failed to scan {path:?} for rebuild tracking: {error}")
        });

        for entry in entries {
            let entry = entry.unwrap_or_else(|error| {
                panic!("failed to read an entry from {path:?} for rebuild tracking: {error}")
            });
            watch_tree(&entry.path());
        }
    } else {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
