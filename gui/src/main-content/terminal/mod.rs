//! Win32 native terminal reparenting controller.
//!
//! Spawns native Windows PowerShell console windows (conhost), strips their frames,
//! borders, and menu bars, and reparents their HWND handles directly as child windows
//! inside the Slint layout viewport. Resizes and moves them dynamically to follow scale
//! and size updates.

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::state::AppState;
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_STYLE, GetParent as Win32GetParent, GetWindowLongW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, MoveWindow, SW_HIDE, SW_SHOW, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetParent, SetWindowLongW, SetWindowPos,
    ShowWindow, WS_BORDER, WS_CAPTION, WS_CHILD, WS_POPUP, WS_THICKFRAME, WS_VISIBLE,
};

extern "system" {
    /// Safe FFI declaration for Win32 SetFocus
    fn SetFocus(hwnd: HWND) -> HWND;
    /// Safe FFI declaration for Win32 AttachThreadInput
    fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: BOOL) -> BOOL;
    /// Safe FFI declaration for Win32 GetCurrentThreadId
    fn GetCurrentThreadId() -> u32;
    /// Safe FFI declaration for Win32 SetWindowLongPtrW to hook WndProc subclassing
    fn SetWindowLongPtrW(hwnd: HWND, nindex: i32, dwnewlong: isize) -> isize;
    /// Safe FFI declaration for Win32 CallWindowProcW to chain subclassed messages
    fn CallWindowProcW(
        prevwndproc: isize,
        hwnd: HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
    /// Safe FFI declaration for Win32 GetCursorPos
    fn GetCursorPos(lppoint: *mut POINT) -> BOOL;
    /// Safe FFI declaration for Win32 ScreenToClient
    fn ScreenToClient(hwnd: HWND, lppoint: *mut POINT) -> BOOL;
}

/// Global atomic storing the original Slint window procedure pointer.
/// Used to forward messages we don't handle in our custom subclass procedure.
static PREV_WND_PROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Physical Y-coordinate threshold of the top edge of the terminal panel.
/// If a click's Y-coordinate is greater than or equal to this threshold, it is inside the terminal panel.
static TERM_Y_THRESHOLD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(99999);

/// Window handle (HWND) of the currently active/focused terminal conhost window.
static ACTIVE_TERM_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Custom window procedure to subclass the parent Slint window.
/// Intercepts mouse clicks and activation messages.
/// If the click falls inside the terminal panel area (Y >= TERM_Y_THRESHOLD), it routes focus to the active terminal conhost window.
/// If the click falls outside (above it, in the chat viewport or sidebar), it pulls focus back to the Slint parent window.
unsafe extern "system" fn terminal_parent_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    // WM_LBUTTONDOWN (0x0201), WM_RBUTTONDOWN (0x0204), WM_MBUTTONDOWN (0x0207), WM_MOUSEACTIVATE (0x0021)
    if msg == 0x0201 || msg == 0x0204 || msg == 0x0207 || msg == 0x0021 {
        let active_hwnd = ACTIVE_TERM_HWND.load(Ordering::SeqCst) as HWND;

        if !active_hwnd.is_null() {
            let mut is_click_inside_terminal = false;

            if msg == 0x0201 || msg == 0x0204 || msg == 0x0207 {
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                let threshold = TERM_Y_THRESHOLD.load(Ordering::SeqCst);
                if y >= threshold {
                    is_click_inside_terminal = true;
                }
            } else if msg == 0x0021 {
                // WM_MOUSEACTIVATE: query screen cursor position and map to client coordinates
                let mut pt = POINT { x: 0, y: 0 };
                if GetCursorPos(&mut pt) != 0 {
                    ScreenToClient(hwnd, &mut pt);
                    let threshold = TERM_Y_THRESHOLD.load(Ordering::SeqCst);
                    if pt.y >= threshold {
                        is_click_inside_terminal = true;
                    }
                }
            }

            if is_click_inside_terminal {
                // Click is inside the terminal panel area (tabs or drag handle) -> route focus to conhost
                focus_terminal_hwnd(active_hwnd);
            } else {
                // Click is outside the terminal panel (chat, sidebar, input panel) -> restore focus to Slint
                SetFocus(hwnd);
            }
        } else {
            // No active terminal, restore focus to Slint
            SetFocus(hwnd);
        }
    }

    let prev = PREV_WND_PROC.load(Ordering::SeqCst);
    if prev != 0 {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        0
    }
}

/// Shared structure containing all active native console handles.
pub struct Win32Terminal {
    /// Process handle of the spawned terminal child.
    pub process: std::process::Child,
    /// Win32 window handle of the console host (`conhost.exe`).
    pub hwnd: HWND,
    /// Display name of the tab.
    pub tab_name: String,
}

// Manually implement Send/Sync since raw HWND pointers are thread-safe to transfer/operate on Windows.
unsafe impl Send for Win32Terminal {}
unsafe impl Sync for Win32Terminal {}

impl Drop for Win32Terminal {
    fn drop(&mut self) {
        // Ensure child process is terminated when dropped to prevent orphaned console hosts
        let _ = self.process.kill();
    }
}

/// Global thread-safe terminal sessions registry.
pub static ACTIVE_TERMINALS: OnceLock<Mutex<HashMap<String, Win32Terminal>>> = OnceLock::new();

/// Atomic session counter used for unique terminal identifier suffixes.
static TAB_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// Retrieves the active terminal registry mutex.
fn get_active_terminals() -> &'static Mutex<HashMap<String, Win32Terminal>> {
    ACTIVE_TERMINALS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct FindTitleData {
    substring: String,
    hwnd: Option<HWND>,
}

unsafe extern "system" fn enum_title_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = &mut *(lparam as *mut FindTitleData);
    let mut title_buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);
    if len > 0 {
        let title_str = String::from_utf16_lossy(&title_buf[..len as usize]);
        if title_str.contains(&data.substring) {
            data.hwnd = Some(hwnd);
            return 0; // stop enumeration
        }
    }
    1 // continue enumeration
}

fn find_hwnd_by_title(substring: &str) -> Option<HWND> {
    let mut data = FindTitleData {
        substring: substring.to_string(),
        hwnd: None,
    };
    unsafe {
        EnumWindows(
            Some(enum_title_proc),
            &mut data as *mut FindTitleData as LPARAM,
        );
    }
    data.hwnd
}

struct FindOwnerData {
    /// The target process ID to match.
    pid: u32,
    /// The window handle found, if any.
    hwnd: Option<HWND>,
}

/// Callback function invoked by `EnumWindows`. It inspects each window to find
/// the main visible application window belonging to our process.
/// Winit creates hidden/utility windows for event loops and message polling.
/// To avoid reparenting to a hidden helper window, we explicitly check that
/// the window is visible on screen, has the exact title "Operon", and has no parent.
unsafe extern "system" fn enum_owner_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = &mut *(lparam as *mut FindOwnerData);
    let mut pid = 0;

    // Get the process ID that owns this window handle.
    GetWindowThreadProcessId(hwnd, &mut pid);

    // Check if this window belongs to our GUI process.
    if pid == data.pid {
        let parent = Win32GetParent(hwnd);

        // Ensure this is a top-level window (no parent).
        if parent.is_null() {
            let mut title_buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);

            if len > 0 {
                let title_str = String::from_utf16_lossy(&title_buf[..len as usize]);

                // Check if this window is currently visible to the user.
                let is_visible = IsWindowVisible(hwnd);

                // Match the main application title "Operon" and guarantee visibility.
                if title_str == "Operon" && is_visible != 0 {
                    data.hwnd = Some(hwnd);
                    return 0; // Stop enumerating windows since we found the main window.
                }
            }
        }
    }
    1 // Continue enumerating other windows.
}

/// Finds and returns the HWND of the main visible Slint application window.
fn get_slint_window_hwnd() -> HWND {
    let pid = std::process::id();
    let mut data = FindOwnerData { pid, hwnd: None };
    unsafe {
        // Enumerate all top-level windows on the desktop to find our main window.
        EnumWindows(
            Some(enum_owner_proc),
            &mut data as *mut FindOwnerData as LPARAM,
        );
    }
    data.hwnd.unwrap_or(std::ptr::null_mut())
}

/// Programmatically attaches the current thread's input processing to the target window's thread.
/// Since the console runs in an external conhost.exe process, Windows isolates input queues by default.
/// Sharing input state using AttachThreadInput is the industry-standard Win32 method to successfully
/// direct SetFocus to external reparented child processes, ensuring keyboard entry is received correctly.
unsafe fn focus_terminal_hwnd(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }

    // Retrieve the thread ID that owns the target console window.
    let target_thread_id = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
    // Get the current GUI application thread ID.
    let current_thread_id = GetCurrentThreadId();

    // If the window belongs to a different thread, attach input queues temporarily, set focus, and detach.
    if current_thread_id != target_thread_id {
        AttachThreadInput(current_thread_id, target_thread_id, 1);
        SetFocus(hwnd);
        AttachThreadInput(current_thread_id, target_thread_id, 0);
    } else {
        SetFocus(hwnd);
    }
}

/// Dynamic terminal tab creation function.
async fn create_new_terminal_tab(
    win_weak: &slint::Weak<crate::OperonWindow>,
    project_dir: Option<String>,
) {
    let session_idx = TAB_COUNTER.fetch_add(1, Ordering::SeqCst);
    let tab_id = format!("term_{}", session_idx);
    let tab_name = format!("pwsh {}", session_idx);

    let workdir = if let Some(dir) = project_dir {
        Some(dir)
    } else if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
        Some(paths.workspace_dir.to_string_lossy().to_string())
    } else {
        None
    };

    // Spawn PowerShell console wrapped in conhost.exe in a background thread to prevent blocking.
    // By invoking conhost.exe explicitly, we bypass any system-wide Windows 11 default terminal
    // setting (which would otherwise open it in Windows Terminal where reparenting is blocked/unsupported).
    let win_w = win_weak.clone();
    tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("conhost.exe");
        if let Some(ref dir) = workdir {
            cmd.current_dir(dir);
        }

        // Spawn with CREATE_NEW_CONSOLE to guarantee conhost window creation
        cmd.creation_flags(0x00000010);

        // Set unique console window title via PowerShell argument.
        let unique_title = format!("OperonTerminal_{}", session_idx);
        let title_command = format!(
            "$Host.UI.RawUI.WindowTitle = '{}'; [System.Console]::Title = '{}'",
            unique_title, unique_title
        );
        cmd.args(&[
            "powershell.exe",
            "-NoExit",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &title_command,
        ]);

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();

                // Wait for the window handle to be allocated by the OS and titled.
                // We use up to 100 retries (5 seconds) as PowerShell startup can be slow.
                let mut console_hwnd = std::ptr::null_mut();
                for _ in 0..100 {
                    if let Some(hwnd) = find_hwnd_by_title(&unique_title) {
                        console_hwnd = hwnd;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                if console_hwnd.is_null() {
                    eprintln!(
                        "[operon-gui][terminal] Failed to find HWND for titled console '{}' (PID {})",
                        unique_title, pid
                    );
                    return;
                }

                // Retrieve the main Slint window HWND using process enum with retry
                let mut parent_hwnd = std::ptr::null_mut();
                for _ in 0..40 {
                    let hwnd = get_slint_window_hwnd();
                    if !hwnd.is_null() {
                        parent_hwnd = hwnd;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                if parent_hwnd.is_null() {
                    eprintln!("[operon-gui][terminal] Parent window HWND not found");
                    return;
                }

                // Subclass the main Slint window procedure exactly once to intercept focus events.
                // This ensures clicking outside the terminal correctly shifts focus back to Slint inputs.
                static SUBCLASSED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !SUBCLASSED.swap(true, Ordering::SeqCst) {
                    unsafe {
                        // GWL_WNDPROC / GWLP_WNDPROC index is -4.
                        // Replacing the window procedure hooks into parent click notifications.
                        let prev = SetWindowLongPtrW(
                            parent_hwnd,
                            -4,
                            terminal_parent_wnd_proc as *const () as isize,
                        );
                        PREV_WND_PROC.store(prev, Ordering::SeqCst);
                    }
                }

                // Strip console frames, borders, and set parent to Slint window
                unsafe {
                    let mut style = GetWindowLongW(console_hwnd, GWL_STYLE) as u32;
                    // Clear popup style so it acts as a child window
                    style &= !WS_POPUP;
                    // Clear borders, titlebar, and thick resizing frame
                    style &= !WS_BORDER;
                    style &= !WS_CAPTION;
                    style &= !WS_THICKFRAME;
                    // Set child and visible styles
                    style |= WS_CHILD;
                    style |= WS_VISIBLE;

                    SetWindowLongW(console_hwnd, GWL_STYLE, style as i32);
                    SetParent(console_hwnd, parent_hwnd);

                    // Force the OS to update window frame style changes immediately
                    SetWindowPos(
                        console_hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );

                    ShowWindow(console_hwnd, SW_SHOW);
                }

                let term = Win32Terminal {
                    process: child,
                    hwnd: console_hwnd,
                    tab_name,
                };

                {
                    get_active_terminals().lock().unwrap().insert(tab_id, term);
                }

                refresh_tabs(&win_w);
            }
            Err(e) => {
                eprintln!("[operon-gui][terminal] Failed to spawn PowerShell: {}", e);
            }
        }
    });
}

/// Refreshes the Slint component properties thread-safely.
fn refresh_tabs(win_weak: &slint::Weak<crate::OperonWindow>) {
    let win_w = win_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(win) = win_w.upgrade() {
            let active_terminals = get_active_terminals().lock().unwrap();
            let mut tabs: Vec<(String, String)> = active_terminals
                .iter()
                .map(|(id, data)| (id.clone(), data.tab_name.clone()))
                .collect();
            // Alphabetical tab order
            tabs.sort_by(|a, b| a.0.cmp(&b.0));

            let names: Vec<SharedString> = tabs
                .iter()
                .map(|(_, name)| SharedString::from(name.clone()))
                .collect();
            win.set_terminal_tab_names(ModelRc::from(Rc::new(VecModel::from(names))));

            let active_idx = win.get_terminal_active_tab();

            let new_active_idx = if tabs.is_empty() {
                -1
            } else if active_idx < 0 || active_idx >= tabs.len() as i32 {
                (tabs.len() - 1) as i32
            } else {
                active_idx
            };

            win.set_terminal_active_tab(new_active_idx);

            if new_active_idx < 0 {
                win.set_is_terminal_open(false);
            }
        }
    });
}

fn sync_terminals_layout(layout_data: (u32, u32, f32, bool, i32, bool, f32, f32)) {
    let (w_width, w_height, scale, is_open, active_tab, sb_open, sb_width, term_height) =
        layout_data;

    let active_terminals = get_active_terminals().lock().unwrap();
    let mut tabs: Vec<String> = active_terminals.keys().cloned().collect();
    tabs.sort();

    // Calculate position in physical pixels
    let logical_sb_width = if sb_open { sb_width } else { 0.0 };
    let physical_sb_width = (logical_sb_width * scale) as i32;
    // Clamp the terminal height logically to match the dynamic Slint limits.
    // This prevents the terminal panel from overflowing when the window is resized.
    let max_logical_height = (w_height as f32 / scale) * 0.75;
    let min_logical_height = 120.0;
    let clamped_term_height = term_height.clamp(
        min_logical_height,
        max_logical_height.max(min_logical_height),
    );

    let physical_term_height = (clamped_term_height * scale) as i32;
    let physical_tab_bar_height = (32.0 * scale) as i32;

    // Set the Y coordinate threshold: if terminal is open, it is the top edge. Otherwise it is unreachable.
    let y_threshold = if is_open {
        (w_height as i32) - physical_term_height
    } else {
        99999
    };
    TERM_Y_THRESHOLD.store(y_threshold, Ordering::SeqCst);

    let x = physical_sb_width;
    let y = (w_height as i32) - physical_term_height + physical_tab_bar_height;
    let width = (w_width as i32) - physical_sb_width;
    let height = physical_term_height - physical_tab_bar_height;

    let mut active_hwnd = 0;
    for (idx, id) in tabs.iter().enumerate() {
        if let Some(term) = active_terminals.get(id) {
            let is_active = is_open && (active_tab == idx as i32);
            if is_active {
                active_hwnd = term.hwnd as isize;
                unsafe {
                    // Update size and position of active terminal window
                    MoveWindow(term.hwnd, x, y, width, height, 1);
                    ShowWindow(term.hwnd, SW_SHOW);

                    // Route focus to the terminal conhost HWND on the Slint main event loop thread.
                    // Since raw pointers like HWND are not Send, we cast it to isize to safely pass
                    // it across the thread boundary into the event loop closure, then cast it back.
                    let hwnd_val = term.hwnd as isize;
                    let _ = slint::invoke_from_event_loop(move || {
                        focus_terminal_hwnd(hwnd_val as HWND);
                    });
                }
            } else {
                unsafe {
                    ShowWindow(term.hwnd, SW_HIDE);

                    // If this terminal session was hidden (collapsed or tab switched),
                    // pull focus back to the main Slint window.
                    let parent_hwnd = Win32GetParent(term.hwnd);
                    if !parent_hwnd.is_null() {
                        let parent_val = parent_hwnd as isize;
                        let _ = slint::invoke_from_event_loop(move || {
                            SetFocus(parent_val as HWND);
                        });
                    }
                }
            }
        }
    }
    ACTIVE_TERM_HWND.store(active_hwnd, Ordering::SeqCst);
}

/// Wires the terminal callbacks from the Slint template interface to the Rust controllers.
pub fn wire_terminal(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    // 1. Tab select callback
    let window_weak_1 = window.as_weak();
    window.on_terminal_tab_clicked(move |idx| {
        let win_w = window_weak_1.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(win) = win_w.upgrade() {
                win.set_terminal_active_tab(idx);
            }
        });
    });

    // 2. Tab close callback
    let window_weak_2 = window.as_weak();
    window.on_terminal_tab_close_clicked(move |idx| {
        let win_w = window_weak_2.clone();
        tokio::spawn(async move {
            let tab_id_to_close = {
                let active_terminals = get_active_terminals().lock().unwrap();
                let mut tabs: Vec<String> = active_terminals.keys().cloned().collect();
                tabs.sort();
                tabs.get(idx as usize).cloned()
            };

            if let Some(id) = tab_id_to_close {
                {
                    get_active_terminals().lock().unwrap().remove(&id);
                }
                refresh_tabs(&win_w);
            }
        });
    });

    // 3. New tab callback
    let window_weak_3 = window.as_weak();
    let state_new = Rc::clone(&state);
    window.on_terminal_tab_new_clicked(move || {
        let win_w = window_weak_3.clone();
        let project_dir = state_new.borrow().current_project_dir().map(String::from);
        tokio::spawn(async move {
            create_new_terminal_tab(&win_w, project_dir).await;
        });
    });

    // We do not need terminal_key_pressed callback since keyboard is routed by Windows OS to the child console directly
    window.on_terminal_key_pressed(|_, _| {});

    // Spawn the background thread to handle layout sync and window resizing
    let window_weak_loop = window.as_weak();
    std::thread::spawn(move || {
        let mut last_state = None;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(30));

            let (tx, rx) = std::sync::mpsc::channel();
            let win_weak_clone = window_weak_loop.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let res = if let Some(win) = win_weak_clone.upgrade() {
                    let size = win.window().size();
                    let scale_factor = win.window().scale_factor();

                    let is_open = win.get_is_terminal_open();
                    let active_tab = win.get_terminal_active_tab();
                    let terminal_height = win.get_terminal_height();
                    let sidebar_open = win.get_sidebar_open();
                    let sidebar_width = win.get_sidebar_width();

                    Some((
                        size.width,
                        size.height,
                        scale_factor,
                        is_open,
                        active_tab,
                        sidebar_open,
                        sidebar_width,
                        terminal_height,
                    ))
                } else {
                    None
                };
                let _ = tx.send(res);
            });

            let layout_data = match rx.recv() {
                Ok(Some(data)) => data,
                _ => {
                    // Slint window was closed, exit thread
                    break;
                }
            };

            let current_state = layout_data;
            if last_state != Some(current_state) {
                last_state = Some(current_state);
                sync_terminals_layout(layout_data);
            }
        }
    });
}
