//! # operon-terminal
//! 
//! Independent backend library for managing pseudo-terminal (PTY) sessions
//! in Operon. It handles spawning shell processes (specifically PowerShell on Windows),
//! reading outputs asynchronously in a background thread, and writing/resizing controls.

use std::io::Write;
use std::sync::{Arc, Mutex};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

/// A handle to a running terminal process.
/// Allows concurrent thread-safe writing and resizing.
pub struct TerminalSession {
    /// Unique identifier for this terminal instance.
    pub id: String,
    
    /// Thread-safe writer pointing to the stdin of the shell process.
    writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    
    /// Thread-safe master PTY reference for calling resize operations.
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
}

impl TerminalSession {
    /// Spawns a new pseudo-terminal session running the native shell.
    ///
    /// # Arguments
    /// - `id`: The unique session identifier (e.g. "term_1").
    /// - `workdir`: Optional directory to start the terminal process in.
    /// - `cols`: Initial width in character columns.
    /// - `rows`: Initial height in character rows.
    /// - `on_output`: Callback invoked when new text/bytes are read from the terminal.
    /// - `on_exit`: Callback invoked when the terminal process exits or EOF is reached.
    pub fn new<F, E>(
        id: String,
        workdir: Option<String>,
        cols: u16,
        rows: u16,
        on_output: F,
        on_exit: E,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str) + Send + Sync + 'static,
        E: Fn() + Send + Sync + 'static,
    {
        // 1. Get the native PTY system implementation (ConPTY on Windows, pty on Unix)
        let pty_system = native_pty_system();
        
        // 2. Open a master/slave pseudo-terminal pair with initial dimensions
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        
        // 3. Determine native shell command (PowerShell on Windows, bash on Unix/macOS)
        let shell = if cfg!(target_os = "windows") {
            "powershell.exe"
        } else {
            "bash"
        };
        
        let mut cmd = CommandBuilder::new(shell);
        
        // If a workspace/project directory is specified, set the start directory
        if let Some(dir) = workdir {
            cmd.cwd(dir);
        }
        
        // 4. Spawn the shell executable into the slave end of the PTY
        let mut child = pair.slave.spawn_command(cmd)?;
        
        // 5. Drop the slave end in this thread, as the spawned process now holds it
        drop(pair.slave);
        
        // 6. Get the writer and clone the reader from the master end
        let master = pair.master;
        let mut reader = master.try_clone_reader()?;
        let writer = master.take_writer()?;
        
        let writer = Arc::new(Mutex::new(writer));
        let master = Arc::new(Mutex::new(master));
        
        // 7. Spawn a background thread to continuously block-read standard output from the PTY
        let id_clone = id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF is reached (shell process was exited by the user)
                        tracing::info!("Terminal session '{}' exited gracefully (EOF)", id_clone);
                        on_exit();
                        break;
                    }
                    Ok(n) => {
                        // Decode raw bytes to UTF-8 lossily and send to the frontend callback
                        let text = String::from_utf8_lossy(&buf[..n]);
                        on_output(&text);
                    }
                    Err(e) => {
                        // Read error (e.g. process terminated forcefully)
                        tracing::error!("Read error on terminal session '{}': {}", id_clone, e);
                        on_exit();
                        break;
                    }
                }
            }
            // Ensure child process is killed on reader thread exit
            let _ = child.kill();
        });
        
        Ok(Self {
            id,
            writer,
            master,
        })
    }
    
    /// Write data (keystrokes or command inputs) to the terminal stdin.
    pub fn write(&self, data: &str) -> std::io::Result<()> {
        let mut w = self.writer.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock terminal stdin writer: {}", e),
            )
        })?;
        w.write_all(data.as_bytes())?;
        w.flush()?;
        Ok(())
    }
    
    /// Resize the layout of the pseudo-terminal window columns and rows.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let m = self.master.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock PTY master for resize: {}", e),
            )
        })?;
        m.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }
}

/// Thread-safe reference-counted pointer type to a terminal session.
pub type TerminalHandle = Arc<TerminalSession>;
