// ============================================================================
// Operon VS Code Extension Host Loader & Native Bridge Client
//
// Hey friend! This is the Extension Host entrypoint that bridges the Webview
// UI with the native Rust JSON-RPC binary (`operon-vscode-bridge`).
//
// Architecture:
// 1. Spawns `operon-vscode-bridge` binary over stdio (JSON lines protocol).
// 2. Dispatches `invoke` requests from both Chat and Settings Webviews to the bridge.
// 3. Receives live streaming events (`stream_token`, `agent-finished`) and broadcasts to Webviews.
// 4. Provides zero-keystroke live-reloading of HTML/CSS/JS during development.
// ============================================================================

const vscode = require('vscode');
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const readline = require('readline');

// Reference to active settings panel in editor space
let activeSettingsPanel = undefined;
let chatProviderInstance = undefined;

/** @type {NativeBridgeClient | null} */
let bridgeClient = null;
let outputChannel = null;

/**
 * Native JSON-RPC Stdio Bridge Client.
 */
class NativeBridgeClient {
  constructor(extensionUri) {
    this.extensionUri = extensionUri;
    this.process = null;
    this.requestId = 1;
    this.pending = new Map();
    this.isStarting = false;
  }

  /**
   * Resolves the executable path of the bridge binary.
   */
  resolveBinaryPath() {
    const isWindows = process.platform === 'win32';
    const binName = isWindows ? 'operon-vscode-bridge.exe' : 'operon-vscode-bridge';
    const rootDir = path.resolve(this.extensionUri.fsPath, '..', '..');

    const candidates = [
      path.join(this.extensionUri.fsPath, 'bin', binName),
      path.join(this.extensionUri.fsPath, '..', 'bridge', 'bin', binName),
      path.join(rootDir, 'target', 'release', binName),
      path.join(rootDir, 'target', 'debug', binName),
    ];

    for (const p of candidates) {
      if (fs.existsSync(p)) {
        return p;
      }
    }

    return null;
  }

  /**
   * Starts or restarts the native bridge child process.
   */
  start() {
    if (this.process || this.isStarting) return;
    this.isStarting = true;

    const binPath = this.resolveBinaryPath();
    if (!binPath) {
      outputChannel?.appendLine('[Operon Bridge] No compiled binary found. Waiting for cargo build.');
      this.isStarting = false;
      return;
    }

    outputChannel?.appendLine(`[Operon Bridge] Spawning native bridge at: ${binPath}`);

    try {
      this.process = spawn(binPath, [], {
        cwd: path.resolve(this.extensionUri.fsPath, '..', '..'),
        stdio: ['pipe', 'pipe', 'pipe'],
        env: {
          ...process.env,
          RUST_LOG: 'info',
        },
      });

      this.isStarting = false;

      const rl = readline.createInterface({
        input: this.process.stdout,
        crlfDelay: Infinity,
      });

      rl.on('line', (line) => {
        const trimmed = line.trim();
        if (!trimmed) return;
        this.handleMessage(trimmed);
      });

      this.process.stderr.on('data', (data) => {
        const text = data.toString('utf8');
        outputChannel?.append(`[Bridge Log] ${text}`);
      });

      this.process.on('close', (code) => {
        outputChannel?.appendLine(`[Operon Bridge] Process exited with code ${code}`);
        this.process = null;
        // Reject all pending requests
        for (const [id, req] of this.pending.entries()) {
          req.reject(new Error(`Bridge process terminated with code ${code}`));
        }
        this.pending.clear();
      });

      this.process.on('error', (err) => {
        outputChannel?.appendLine(`[Operon Bridge] Process error: ${err.message}`);
        this.process = null;
      });
    } catch (err) {
      outputChannel?.appendLine(`[Operon Bridge] Spawn failed: ${err.message}`);
      this.isStarting = false;
    }
  }

  /**
   * Dispatches a JSON-RPC request to the bridge binary over stdin.
   */
  invoke(method, params = {}) {
    if (!this.process) {
      this.start();
    }

    if (!this.process) {
      return Promise.reject(new Error('Native bridge binary is not running. Please build with cargo.'));
    }

    const id = this.requestId++;
    const payload = {
      jsonrpc: '2.0',
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error(`Bridge request '${method}' timed out after 30s`));
        }
      }, 30000);

      this.pending.set(id, { resolve, reject, timer });

      try {
        this.process.stdin.write(JSON.stringify(payload) + '\n');
      } catch (err) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(err);
      }
    });
  }

  /**
   * Handles incoming JSON lines from the bridge's stdout.
   */
  handleMessage(jsonString) {
    try {
      const msg = JSON.parse(jsonString);

      // 1. JSON-RPC Response matching a pending request
      if (msg.id !== undefined && msg.id !== null) {
        const req = this.pending.get(msg.id);
        if (req) {
          clearTimeout(req.timer);
          this.pending.delete(msg.id);

          if (msg.error) {
            req.reject(new Error(msg.error.message || JSON.stringify(msg.error)));
          } else {
            req.resolve(msg.result);
          }
        }
        return;
      }

      // 2. Streaming Event Notification
      if (msg.method === 'operon://stream-event' && msg.params) {
        const { event, payload } = msg.params;
        this.broadcastEvent(event, payload);
      }
    } catch (err) {
      outputChannel?.appendLine(`[Operon Bridge] Parse error for stdout: ${err.message}`);
    }
  }

  /**
   * Broadcasts events to all connected webviews.
   */
  broadcastEvent(event, payload) {
    const msg = { type: 'event', event, payload };
    chatProviderInstance?.postMessage(msg);
    if (activeSettingsPanel) {
      activeSettingsPanel.webview.postMessage(msg);
    }
  }

  stop() {
    if (this.process) {
      this.process.kill();
      this.process = null;
    }
  }
}

/**
 * @param {vscode.ExtensionContext} context
 */
function activate(context) {
  outputChannel = vscode.window.createOutputChannel('Operon Native Bridge');
  outputChannel.appendLine('[Operon] Extension host activating...');

  bridgeClient = new NativeBridgeClient(context.extensionUri);
  bridgeClient.start();

  const provider = new OperonChatViewProvider(context.extensionUri);
  chatProviderInstance = provider;

  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider('operon.chatView', provider, {
      webviewOptions: {
        retainContextWhenHidden: true,
      },
    })
  );

  // Register commands
  context.subscriptions.push(
    vscode.commands.registerCommand('operon.openChat', () => {
      vscode.commands.executeCommand('operon.chatView.focus');
    }),

    vscode.commands.registerCommand('operon.newSession', () => {
      provider.postMessage({ type: 'event', event: 'new-session', payload: {} });
    }),

    vscode.commands.registerCommand('operon.cancelPrompt', () => {
      bridgeClient?.invoke('cancel_prompt');
    }),

    vscode.commands.registerCommand('operon.openSettings', () => {
      openSettingsTab(context.extensionUri);
    })
  );
}

/**
 * Opens or focuses the Operon Settings panel as a tab in the editor area.
 * @param {vscode.Uri} extensionUri
 */
function openSettingsTab(extensionUri) {
  if (activeSettingsPanel) {
    activeSettingsPanel.reveal(vscode.ViewColumn.Active);
    return;
  }

  activeSettingsPanel = vscode.window.createWebviewPanel(
    'operon.settings',
    'Operon Settings',
    vscode.ViewColumn.Active,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [vscode.Uri.joinPath(extensionUri, 'src')],
    }
  );

  try {
    activeSettingsPanel.iconPath = vscode.Uri.joinPath(extensionUri, 'src', 'assets', 'brand', 'icon.png');
  } catch (_) {}

  renderSettingsHtml(activeSettingsPanel, extensionUri);

  // Live reload for settings panel
  const vsWatcher = vscode.workspace.createFileSystemWatcher(
    new vscode.RelativePattern(extensionUri, 'src/**/*')
  );

  const reloadSettings = () => {
    if (activeSettingsPanel) {
      renderSettingsHtml(activeSettingsPanel, extensionUri);
    }
  };

  vsWatcher.onDidChange(reloadSettings);
  vsWatcher.onDidCreate(reloadSettings);
  vsWatcher.onDidDelete(reloadSettings);

  // Handle IPC calls from the Settings Webview
  activeSettingsPanel.webview.onDidReceiveMessage(async (msg) => {
    if (!msg || typeof msg !== 'object') return;

    if (msg.type === 'invoke') {
      try {
        const result = await handleUnifiedIpcInvoke(msg.cmd, msg.args, extensionUri);
        activeSettingsPanel.webview.postMessage({
          id: msg.id,
          type: 'response',
          result: result,
        });
      } catch (err) {
        activeSettingsPanel.webview.postMessage({
          id: msg.id,
          type: 'response',
          error: err && err.message ? err.message : String(err),
        });
      }
    }
  });

  activeSettingsPanel.onDidDispose(() => {
    vsWatcher.dispose();
    activeSettingsPanel = undefined;
  });
}

/**
 * Reads and renders `src/settings.html` into a Webview Panel.
 * @param {vscode.WebviewPanel} panel
 * @param {vscode.Uri} extensionUri
 */
function renderSettingsHtml(panel, extensionUri) {
  const webview = panel.webview;
  const settingsPath = path.join(extensionUri.fsPath, 'src', 'settings.html');

  try {
    let html = fs.readFileSync(settingsPath, 'utf8');
    const timestamp = Date.now();
    const srcBaseUri = webview.asWebviewUri(vscode.Uri.joinPath(extensionUri, 'src'));

    const baseTag = `<base href="${srcBaseUri}/">`;
    const cspTag = `
      <meta http-equiv="Content-Security-Policy" content="
        default-src 'none';
        img-src ${webview.cspSource} https: data:;
        font-src ${webview.cspSource} data:;
        style-src ${webview.cspSource} 'unsafe-inline';
        script-src ${webview.cspSource} 'unsafe-inline' 'unsafe-eval';
      ">
    `;

    html = html.replace(/src="js\/settings\/settings\.js"/g, `src="js/settings/settings.js?v=${timestamp}"`);
    html = html.replace(/href="css\/(.+?)\.css"/g, `href="css/$1.css?v=${timestamp}"`);
    html = html.replace('<head>', `<head>\n  ${baseTag}\n  ${cspTag}`);

    panel.webview.html = html;
  } catch (err) {
    console.error('[Operon] Error reading src/settings.html:', err);
    panel.webview.html = `<div style="padding: 24px; color: red;">Error loading Operon Settings: ${err.message}</div>`;
  }
}

/**
 * Universal IPC dispatcher handling UI actions & routing all backend queries to Rust bridge.
 */
async function handleUnifiedIpcInvoke(cmd, args, extensionUri) {
  // 1. UI-level actions handled directly by VS Code Extension Host
  switch (cmd) {
    case 'open_settings_window':
    case 'open_settings':
      openSettingsTab(extensionUri);
      return null;

    case 'close_settings_window':
      if (activeSettingsPanel) {
        activeSettingsPanel.dispose();
      }
      return null;

    case 'pick_allowed_directory_dialog':
    case 'open_project_picker': {
      const uri = await vscode.window.showOpenDialog({
        canSelectFiles: false,
        canSelectFolders: true,
        canSelectMany: false,
        openLabel: 'Select Folder',
      });
      return uri && uri[0] ? uri[0].fsPath : null;
    }

    case 'open_external_url':
      if (args && args.url) {
        vscode.env.openExternal(vscode.Uri.parse(args.url));
      }
      return null;
  }

  // 2. Delegate everything else to the Rust JSON-RPC Bridge
  if (bridgeClient) {
    try {
      return await bridgeClient.invoke(cmd, args || {});
    } catch (err) {
      outputChannel?.appendLine(`[Operon Bridge] Error handling '${cmd}': ${err.message}`);
      throw err;
    }
  }

  throw new Error('Bridge client not available');
}

class OperonChatViewProvider {
  /**
   * @param {vscode.Uri} extensionUri
   */
  constructor(extensionUri) {
    this.extensionUri = extensionUri;
    this.view = undefined;
    this.reloadTimer = null;
  }

  /**
   * @param {vscode.WebviewView} webviewView
   * @param {vscode.WebviewViewResolveContext} _context
   * @param {vscode.CancellationToken} _token
   */
  resolveWebviewView(webviewView, _context, _token) {
    this.view = webviewView;

    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(this.extensionUri, 'src')],
    };

    this.renderHtml();

    const vsWatcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(this.extensionUri, 'src/**/*')
    );

    vsWatcher.onDidChange(() => this.scheduleLiveReload());
    vsWatcher.onDidCreate(() => this.scheduleLiveReload());
    vsWatcher.onDidDelete(() => this.scheduleLiveReload());

    webviewView.onDidDispose(() => {
      vsWatcher.dispose();
    });

    webviewView.webview.onDidReceiveMessage(async (msg) => {
      if (!msg || typeof msg !== 'object') return;

      if (msg.type === 'invoke') {
        try {
          const result = await handleUnifiedIpcInvoke(msg.cmd, msg.args, this.extensionUri);
          this.postMessage({
            id: msg.id,
            type: 'response',
            result: result,
          });
        } catch (err) {
          this.postMessage({
            id: msg.id,
            type: 'response',
            error: err && err.message ? err.message : String(err),
          });
        }
      }
    });
  }

  scheduleLiveReload() {
    if (this.reloadTimer) {
      clearTimeout(this.reloadTimer);
    }
    this.reloadTimer = setTimeout(() => {
      this.renderHtml();
    }, 50);
  }

  postMessage(msg) {
    if (this.view) {
      this.view.webview.postMessage(msg);
    }
  }

  renderHtml() {
    if (!this.view) return;

    const webview = this.view.webview;
    const indexPath = path.join(this.extensionUri.fsPath, 'src', 'index.html');

    try {
      let html = fs.readFileSync(indexPath, 'utf8');
      const timestamp = Date.now();
      const srcBaseUri = webview.asWebviewUri(vscode.Uri.joinPath(this.extensionUri, 'src'));

      const baseTag = `<base href="${srcBaseUri}/">`;
      const cspTag = `
        <meta http-equiv="Content-Security-Policy" content="
          default-src 'none';
          img-src ${webview.cspSource} https: data:;
          font-src ${webview.cspSource} data:;
          style-src ${webview.cspSource} 'unsafe-inline';
          script-src ${webview.cspSource} 'unsafe-inline' 'unsafe-eval';
        ">
      `;

      html = html.replace(/src="js\/main\.js"/g, `src="js/main.js?v=${timestamp}"`);
      html = html.replace(/href="css\/(.+?)\.css"/g, `href="css/$1.css?v=${timestamp}"`);
      html = html.replace('<head>', `<head>\n  ${baseTag}\n  ${cspTag}`);

      this.view.webview.html = html;
    } catch (err) {
      console.error('[Operon] Error reading src/index.html:', err);
      this.view.webview.html = `<div style="padding: 16px; color: red;">Error loading Operon index.html: ${err.message}</div>`;
    }
  }
}

function deactivate() {
  if (bridgeClient) {
    bridgeClient.stop();
  }
  console.log('[Operon] Extension host deactivated.');
}

module.exports = {
  activate,
  deactivate,
};
