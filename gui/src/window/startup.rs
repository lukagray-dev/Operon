//! Startup window geometry helpers for the Operon shell.
//!
//! The goal here is simple: keep the initial window size and position
//! deterministic, centered, and comfortable on a laptop display. The math is
//! isolated in a pure helper so it stays easy to test.

use slint::winit_030::{winit, WinitWindowAccessor};
use slint::{ComponentHandle, PhysicalPosition, PhysicalSize, WindowPosition};

const STARTUP_FILL_RATIO: f32 = 0.70;
const TARGET_ASPECT_RATIO: f32 = 16.0 / 9.0;

/// Concrete geometry for the window before the first frame is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupGeometry {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

/// Calculates a centered 16:9 window size that fits inside a comfortable
/// portion of the monitor.
///
/// The returned window is sized to roughly 70% of the available monitor area
/// while preserving the requested landscape aspect ratio.
pub fn calculate_startup_geometry(
    monitor_width: u32,
    monitor_height: u32,
    monitor_x: i32,
    monitor_y: i32,
) -> StartupGeometry {
    let usable_width = (monitor_width as f32 * STARTUP_FILL_RATIO).round().max(1.0);
    let usable_height = (monitor_height as f32 * STARTUP_FILL_RATIO)
        .round()
        .max(1.0);

    let width_from_height = usable_height * TARGET_ASPECT_RATIO;

    let (window_width, window_height) = if width_from_height <= usable_width {
        (width_from_height, usable_height)
    } else {
        (usable_width, usable_width / TARGET_ASPECT_RATIO)
    };

    let width = window_width.round().max(1.0) as u32;
    let height = window_height.round().max(1.0) as u32;

    let x = monitor_x + ((monitor_width as i32 - width as i32) / 2);
    let y = monitor_y + ((monitor_height as i32 - height as i32) / 2);

    StartupGeometry {
        width,
        height,
        x,
        y,
    }
}

/// Applies the initial size and position before the window is shown.
///
/// If the backend cannot tell us which monitor the window belongs to yet, we
/// keep the existing fallback geometry and log the situation instead of
/// inventing coordinates.
pub fn apply_startup_geometry(app: &crate::OperonWindow) {
    let geometry = app
        .window()
        .with_winit_window(|winit_window: &winit::window::Window| {
            let monitor = winit_window
                .current_monitor()
                .or_else(|| winit_window.primary_monitor())
                .or_else(|| winit_window.available_monitors().next())?;

            let monitor_size = monitor.size();
            let monitor_position = monitor.position();

            Some(calculate_startup_geometry(
                monitor_size.width,
                monitor_size.height,
                monitor_position.x,
                monitor_position.y,
            ))
        });

    match geometry {
        Some(Some(geometry)) => {
            eprintln!(
                "[operon-gui][window] Startup geometry resolved: window={}x{} at ({}, {})",
                geometry.width, geometry.height, geometry.x, geometry.y
            );

            let window = app.window();
            window.set_size(PhysicalSize::new(geometry.width, geometry.height));
            window.set_position(WindowPosition::Physical(PhysicalPosition::new(
                geometry.x, geometry.y,
            )));
        }
        Some(None) => {
            eprintln!(
                "[operon-gui][window] No monitor could be detected for the startup geometry; keeping the default size and position."
            );
        }
        None => {
            eprintln!(
                "[operon-gui][window] The backend did not expose a winit window, so the startup geometry could not be adjusted."
            );
        }
    }
}
