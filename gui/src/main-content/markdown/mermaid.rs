// =============================================================================
// Markdown Mermaid Diagram Engine (`src/markdown/mermaid.rs`)
// =============================================================================
// Formatter and headless SVG renderer for Mermaid diagrams (```mermaid ... ```).
// Uses `merman::render::HeadlessRenderer` to compile Mermaid syntax directly into
// SVG vector data, loaded natively by Slint's `slint::Image`.

use merman::render::HeadlessRenderer;
use slint::Image;

/// Renders a raw Mermaid diagram code string into a Slint `Image`.
/// On parse or layout error, returns a default empty Image rather than panicking.
pub fn render_mermaid_diagram(code: &str) -> Image {
    let renderer = HeadlessRenderer::new().with_diagram_id("operon-mermaid");
    match renderer.render_svg_sync(code) {
        Ok(Some(svg_str)) => Image::load_from_svg_data(svg_str.as_bytes()).unwrap_or_default(),
        _ => Image::default(),
    }
}
