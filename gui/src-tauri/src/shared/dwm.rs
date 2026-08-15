//! DWM Window Styling Helpers for Windows.
// Ensures sharp rectangular corners and matching border color without light outlines.

use tauri::WebviewWindow;

/// Applies Windows-specific DWM attributes for sharp corners and no light border outline.
pub fn apply_window_dwm_styling(window: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
            DWMWCP_DONOTROUND,
        };

        if let Ok(hwnd) = window.hwnd() {
            let raw_hwnd = hwnd.0 as isize as *mut std::ffi::c_void;

            // 1. Force sharp rectangular corners (DWMWCP_DONOTROUND = 1)
            let corner_pref = DWMWCP_DONOTROUND;
            unsafe {
                DwmSetWindowAttribute(
                    raw_hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                    &corner_pref as *const _ as *const _,
                    std::mem::size_of_val(&corner_pref) as u32,
                );

                // 2. Remove white outline by matching border color to titlebar background (#191919)
                let border_color: u32 = 0x00191919;
                DwmSetWindowAttribute(
                    raw_hwnd,
                    DWMWA_BORDER_COLOR as u32,
                    &border_color as *const _ as *const _,
                    std::mem::size_of_val(&border_color) as u32,
                );
            }
        }
    }
}
