// ChatViewProvider — registers and manages the Operon sidebar WebviewView.
//
// VS Code calls `resolveWebviewView()` once when the user opens the Operon
// activity bar panel. After that, the webview lives until VS Code disposes it.
// Because we set `retainContextWhenHidden: true`, the webview stays alive even
// when the user switches to another panel — no need to re-mount the UI.

import * as vscode from "vscode";
import * as path from "path";
import { BridgeClient } from "./bridge";
import { RpcEvent, PermissionReqData, AgentErrorData, AgentFinishedData } from "./rpc";

export class ChatViewProvider implements vscode.WebviewViewProvider {
  private view: vscode.WebviewView | undefined;

  // Active session ID — undefined means a new session will be created on next prompt.
  private activeSessionId: string | undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly bridge: BridgeClient
  ) {}

  // ── WebviewViewProvider ───────────────────────────────────────────────────

  /**
   * Called by VS Code to create the webview view content.
   * Sets up the HTML shell and wires the message bus between
   * the webview (postMessage) and the bridge (RpcEvent stream).
   */
  resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken
  ): void {
    this.view = webviewView;

    webviewView.webview.options = {
      enableScripts: true,
      // Only allow loading resources from the extension's media/ directory
      localResourceRoots: [
        vscode.Uri.joinPath(this.context.extensionUri, "media"),
      ],
    };

    webviewView.webview.html = this.buildHtml(webviewView.webview);

    // Handle messages posted from the webview JavaScript
    webviewView.webview.onDidReceiveMessage(
      (msg: WebviewMessage) => this.handleWebviewMessage(msg),
      undefined,
      this.context.subscriptions
    );
  }

  // ── Public API ────────────────────────────────────────────────────────────

  /** Resets to a new session — called from the "New Session" command. */
  newSession(): void {
    this.activeSessionId = undefined;
    this.postToWebview({ type: "session_reset" });
  }

  // ── Webview → Extension message handler ──────────────────────────────────

  private handleWebviewMessage(msg: WebviewMessage): void {
    switch (msg.type) {
      case "submit_prompt":
        this.handleSubmitPrompt(msg.prompt);
        break;
      case "approve_permission":
        this.bridge.approvePermission(msg.permissionId);
        break;
      case "deny_permission":
        this.bridge.denyPermission(msg.permissionId);
        break;
      case "cancel":
        this.bridge.cancel();
        break;
    }
  }

  private handleSubmitPrompt(prompt: string): void {
    // Resolve the workspace root — use the first open workspace folder if available
    const workspacePath = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

    this.bridge.submitPrompt(
      this.activeSessionId,
      prompt,
      workspacePath,
      (event: RpcEvent) => this.handleBridgeEvent(event)
    );
  }

  // ── Bridge → Webview event router ─────────────────────────────────────────

  /**
   * Routes each RpcEvent from the bridge into a typed message posted to
   * the webview's JavaScript. The webview renders from these messages.
   */
  private handleBridgeEvent(event: RpcEvent): void {
    switch (event.event) {
      case "text_delta":
        this.postToWebview({ type: "text_delta", data: event.data });
        break;

      case "tool_start":
        this.postToWebview({ type: "tool_start", data: event.data });
        break;

      case "tool_result":
        this.postToWebview({ type: "tool_result", data: event.data });
        break;

      case "tool_progress":
        this.postToWebview({ type: "tool_progress", data: event.data });
        break;

      case "permission_req": {
        // Show a VS Code notification AND send to webview for inline rendering
        const req = event.data as PermissionReqData;
        this.postToWebview({ type: "permission_req", data: req });
        vscode.window
          .showInformationMessage(
            `Operon wants to: ${req.description}`,
            "Approve",
            "Deny"
          )
          .then((choice: string | undefined) => {
            if (choice === "Approve") this.bridge.approvePermission(req.permission_id);
            else if (choice === "Deny") this.bridge.denyPermission(req.permission_id);
          });
        break;
      }

      case "token_update":
        this.postToWebview({ type: "token_update", data: event.data });
        break;

      case "agent_finished": {
        const finished = event.data as AgentFinishedData;
        // Persist the session ID so the next prompt continues this conversation
        this.activeSessionId = finished.session_id;
        this.postToWebview({ type: "agent_finished", session_id: finished.session_id });
        break;
      }

      case "agent_error": {
        const err = event.data as AgentErrorData;
        this.postToWebview({ type: "agent_error", message: err.message });
        vscode.window.showErrorMessage(`Operon error: ${err.message}`);
        break;
      }
    }
  }

  // ── HTML shell ────────────────────────────────────────────────────────────

  /**
   * Returns the HTML string for the webview.
   * The actual chat UI JavaScript and CSS live in media/ and are loaded
   * via webview-safe URIs (vscode-resource: scheme).
   */
  private buildHtml(webview: vscode.Webview): string {
    // Convert local file paths to webview-accessible URIs
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, "media", "chat.js")
    );
    const styleUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, "media", "chat.css")
    );

    // Content Security Policy — only allow scripts/styles from our media/ dir
    const nonce = generateNonce();

    return /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta
    http-equiv="Content-Security-Policy"
    content="default-src 'none'; style-src ${webview.cspSource} 'nonce-${nonce}'; script-src 'nonce-${nonce}';"
  />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <link rel="stylesheet" href="${styleUri}" />
  <title>Operon</title>
</head>
<body>
  <div id="root"></div>
  <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  /** Posts a typed message to the webview's JavaScript context. */
  private postToWebview(msg: Record<string, unknown>): void {
    this.view?.webview.postMessage(msg);
  }
}

// ── Types ─────────────────────────────────────────────────────────────────────

/** Union of all messages the webview JavaScript can send to the extension. */
type WebviewMessage =
  | { type: "submit_prompt"; prompt: string }
  | { type: "approve_permission"; permissionId: string }
  | { type: "deny_permission"; permissionId: string }
  | { type: "cancel" };

/** Generates a cryptographically random nonce for CSP. */
function generateNonce(): string {
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  return Array.from({ length: 32 }, () =>
    chars.charAt(Math.floor(Math.random() * chars.length))
  ).join("");
}
