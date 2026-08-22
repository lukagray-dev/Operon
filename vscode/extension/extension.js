// ============================================================================
// Operon VS Code Extension Host Loader
//
// Hey friend! This is the lightweight Extension Host entrypoint that loads
// our Webview UI directly from `src/index.html` (with compiled `src/js/` and `src/css/`).
//
// Features:
// 1. Zero-keystroke instant live-reloading during development
// 2. Cache-busting module reloader on file changes
// 3. Webview IPC communication relay to the native agent backend
// ============================================================================

const vscode = require('vscode');
const fs = require('fs');
const path = require('path');

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
    })
  );
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

    // Enable JavaScript and allow loading local resources from the src/ directory
    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        vscode.Uri.joinPath(this.extensionUri, 'src'),
      ],
    };

    // Render the initial webview HTML
    this.renderHtml();

    // ── Zero-Keystroke Live-Reload System ───────────────────────────────────
    // 1. Native fs.watch directly on src/ for instant zero-latency file watching
    const srcDir = path.join(this.extensionUri.fsPath, 'src');
    try {
      if (fs.existsSync(srcDir)) {
        const fsWatcher = fs.watch(srcDir, { recursive: true }, (_eventType, filename) => {
          if (!filename) return;
          // Ignore temp files or hidden files
          if (filename.includes('.git') || filename.includes('node_modules')) return;
          this.scheduleLiveReload();
        });

        webviewView.onDidDispose(() => {
          try { fsWatcher.close(); } catch (_) {}
        });
      }
    } catch (e) {
      console.warn('[Operon] fs.watch not supported for live reload, falling back to VS Code watcher:', e);
    }

    // 2. VS Code Workspace FileSystemWatcher backup
    const vsWatcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(this.extensionUri, 'src/**/*')
    );

    vsWatcher.onDidChange(() => this.scheduleLiveReload());
    vsWatcher.onDidCreate(() => this.scheduleLiveReload());
    vsWatcher.onDidDelete(() => this.scheduleLiveReload());

    webviewView.onDidDispose(() => {
      vsWatcher.dispose();
    });

    // ── Webview Message Dispatcher ──────────────────────────────────────────
    webviewView.webview.onDidReceiveMessage(async (msg) => {
      if (!msg || typeof msg !== 'object') return;

      // Handle invoke calls from invokeIpc() in shared/ipc.js
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

  /**
   * Debounces reload requests by 50ms so rapid writes from tsc/css edits
   * trigger a single clean reload.
   */
  scheduleLiveReload() {
    if (this.reloadTimer) {
      clearTimeout(this.reloadTimer);
    }
    this.reloadTimer = setTimeout(() => {
      this.renderHtml();
    }, 50);
  }

  /**
   * Dispatches IPC commands invoked by the webview.
   * @param {string} cmd
   * @param {any} args
   */
  async handleIpcInvoke(cmd, args) {
    switch (cmd) {
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

  /**
   * Posts a typed message to the Webview.
   * @param {any} msg
   */
  postMessage(msg) {
    if (this.view) {
      this.view.webview.postMessage(msg);
    }
  }

  /**
   * Reads and renders `src/index.html` with transformed Webview resource URIs
   * and cache-busting module loaders.
   */
  renderHtml() {
    if (!this.view) return;

    const webview = this.view.webview;
    const indexPath = path.join(this.extensionUri.fsPath, 'src', 'index.html');

    try {
      let html = fs.readFileSync(indexPath, 'utf8');

      // Unique timestamp to bust ES module caching on live reloads
      const timestamp = Date.now();

      // Base URI for resolving relative paths (href="css/...", src="js/...", src="assets/...")
      const srcBaseUri = webview.asWebviewUri(vscode.Uri.joinPath(this.extensionUri, 'src'));

      // Injects <base href="..."> so all relative links in index.html resolve properly
      const baseTag = `<base href="${srcBaseUri}/">`;

      // Content Security Policy allowing local assets, scripts, stylesheets, and fonts
      const cspTag = `
        <meta http-equiv="Content-Security-Policy" content="
          default-src 'none';
          img-src ${webview.cspSource} https: data:;
          font-src ${webview.cspSource} data:;
          style-src ${webview.cspSource} 'unsafe-inline';
          script-src ${webview.cspSource} 'unsafe-inline' 'unsafe-eval';
        ">
      `;

      // Transform module script tags to include cache buster so Chromium re-executes new JS
      html = html.replace(/src="js\/main\.js"/g, `src="js/main.js?v=${timestamp}"`);
      html = html.replace(/href="css\/(.+?)\.css"/g, `href="css/$1.css?v=${timestamp}"`);

      // Insert base tag and CSP right after <head>
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
