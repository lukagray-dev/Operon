// ============================================================================
// Operon VS Code Extension Host Loader
//
// Hey friend! This is the lightweight Extension Host entrypoint that loads
// our Webview UI directly from `src/index.html` (with compiled `src/js/` and `src/css/`).
//
// Features:
// 1. Sidebar Webview Provider (Chat Timeline)
// 2. Standalone Editor Tab Webview (Settings Tab in editor space)
// 3. Zero-keystroke instant live-reloading during development
// 4. Cache-busting module reloader on file changes
// 5. Webview IPC communication relay to the native agent backend
// ============================================================================

const vscode = require('vscode');
const fs = require('fs');
const path = require('path');

// Keep reference to active settings editor tab panel to avoid duplicates
let activeSettingsPanel = undefined;

/**
 * @param {vscode.ExtensionContext} context
 */
function activate(context) {
  console.log('[Operon] Extension host activated.');

  const provider = new OperonChatViewProvider(context.extensionUri);

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
      provider.postMessage({ type: 'event', event: 'cancel-prompt', payload: {} });
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
        const result = await handleSettingsIpcInvoke(msg.cmd, msg.args, extensionUri);
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
 * Dispatches IPC commands from the settings tab.
 */
async function handleSettingsIpcInvoke(cmd, args, extensionUri) {
  switch (cmd) {
    case 'get_general_settings':
      return {
        autostart: false,
        minimize_to_tray: false,
        start_minimized: false,
        close_action: 'Exit',
        auto_approve_default: false,
        auto_scroll_stream: true,
        notify_on_permission_request: true,
        notify_on_response_complete: false,
        auto_collapse_reasoning_and_tools: false,
        auto_update_checks: true,
        anonymous_telemetry: false,
      };

    case 'get_appearance_settings':
      return {
        code_theme: 'github-dark',
        show_line_numbers: true,
        highlight_inline_code: true,
        table_style: 'github-dark',
        orb_style: 'composing',
        orb_speed: 'fast',
        show_live_orb: true,
        ui_font: 'Open Sans',
        assistant_font: 'Literata',
        code_font: 'Kode Mono',
      };

    case 'get_models_settings':
      return { providers: [] };

    case 'get_permissions_settings':
      return { allowed_directories: [], global_permissions: [] };

    case 'get_channels_settings':
      return { channels: [] };

    case 'get_memory_settings':
    case 'list_memories':
      return [];

    case 'close_settings_window':
      if (activeSettingsPanel) {
        activeSettingsPanel.dispose();
      }
      return null;

    default:
      console.log(`[Operon Settings Host] Unhandled settings cmd: ${cmd}`, args);
      return null;
  }
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

    // Native fs.watch on src/ for instant zero-latency file watching
    const srcDir = path.join(this.extensionUri.fsPath, 'src');
    try {
      if (fs.existsSync(srcDir)) {
        const fsWatcher = fs.watch(srcDir, { recursive: true }, (_eventType, filename) => {
          if (!filename) return;
          if (filename.includes('.git') || filename.includes('node_modules')) return;
          this.scheduleLiveReload();
        });

        webviewView.onDidDispose(() => {
          try { fsWatcher.close(); } catch (_) {}
        });
      }
    } catch (e) {
      console.warn('[Operon] fs.watch not supported, using VS Code watcher:', e);
    }

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
          const result = await this.handleIpcInvoke(msg.cmd, msg.args);
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

  /**
   * Dispatches IPC commands from the sidebar chat webview.
   */
  async handleIpcInvoke(cmd, args) {
    switch (cmd) {
      case 'open_settings_window':
      case 'open_settings':
        openSettingsTab(this.extensionUri);
        return null;

      case 'get_topbar_info':
        return { title: 'New Session', is_project: false };

      case 'get_available_models':
        return [];

      case 'get_context_window_info':
        return { tokens_used: 0, tokens_total: 128000, percentage: 0, formatted: '0 / 128k' };

      case 'get_git_diff_stats':
        return { insertions: 0, deletions: 0, files_changed: 0, is_git_repo: false };

      case 'load_session_messages':
        return [];

      default:
        console.log(`[Operon Extension Host] Unhandled invoke cmd: ${cmd}`, args);
        return null;
    }
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
  console.log('[Operon] Extension host deactivated.');
}

module.exports = {
  activate,
  deactivate,
};
