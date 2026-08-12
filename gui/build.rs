//! Build script for the GUI crate.
//!
//! Slint compiles the `.slint` files into Rust code at build time, so the build
//! script needs to do two things:
//! - tell Cargo which files should trigger a rebuild
//! - ask `slint-build` to compile the top-level window markup
//! - rasterize the SVG icon into multi-resolution PNGs and an ICO for the system tray

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir_str = env::var("OUT_DIR").expect("OUT_DIR environment variable not set");
    let out_dir = Path::new(&out_dir_str);

    // Watch the UI and asset trees recursively so edits to any imported file
    // trigger a rebuild. That keeps the titlebar and the icons in sync with the
    // generated Rust code.
    watch_tree(&manifest_dir.join("ui"));
    watch_tree(&manifest_dir.join("assets"));

    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("assets/brand/operon.svg").display()
    );

    slint_build::compile("ui/window.slint").expect("failed to compile the Operon Slint UI");

    rasterize_tray_icons(manifest_dir, out_dir);
}

fn rasterize_tray_icons(manifest_dir: &Path, out_dir: &Path) {
    let svg_path = manifest_dir.join("assets/brand/operon.svg");
    let svg_data = fs::read(&svg_path)
        .unwrap_or_else(|err| panic!("failed to read SVG file at {}: {err}", svg_path.display()));

    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&svg_data, &opt)
        .expect("failed to parse SVG data for tray icon");

    let sizes: &[(u32, u32, &str)] = &[
        (16, 16, "tray_icon_16.png"),
        (32, 32, "tray_icon_32.png"),
        (48, 48, "tray_icon_48.png"),
        (256, 256, "tray_icon_256.png"),
    ];

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for &(width, height, filename) in sizes {
        let mut pixmap = tiny_skia::Pixmap::new(width, height)
            .unwrap_or_else(|| panic!("failed to create pixmap of size {width}x{height}"));

        let scale_x = width as f32 / tree.size().width();
        let scale_y = height as f32 / tree.size().height();
        let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let png_data = pixmap.encode_png().expect("failed to encode PNG data");
        let png_path = out_dir.join(filename);
        fs::write(&png_path, &png_data).unwrap_or_else(|err| {
            panic!("failed to write PNG icon to {}: {err}", png_path.display())
        });

        let file = fs::File::open(&png_path)
            .unwrap_or_else(|err| panic!("failed to open PNG file for ICO encoding: {err}"));
        let image = ico::IconImage::read_png(file)
            .unwrap_or_else(|err| panic!("failed to read PNG into ICO image: {err}"));
        let entry = ico::IconDirEntry::encode(&image)
            .unwrap_or_else(|err| panic!("failed to encode ICO dir entry: {err}"));
        icon_dir.add_entry(entry);
    }

    let ico_path = out_dir.join("tray_icon.ico");
    let ico_file = fs::File::create(&ico_path)
        .unwrap_or_else(|err| panic!("failed to create ICO file at {}: {err}", ico_path.display()));
    icon_dir.write(ico_file).unwrap_or_else(|err| {
        panic!(
            "failed to write ICO container to {}: {err}",
            ico_path.display()
        )
    });
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
