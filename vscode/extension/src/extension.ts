// Extension entry point — VS Code calls `activate()` when the extension loads
// and `deactivate()` when it is disabled or VS Code shuts down.

import * as vscode from "vscode";
import { BridgeClient } from "./bridge";
import { ChatViewProvider } from "./panel";

// Single shared bridge instance for the lifetime of the extension.
// The bridge is the child process running the operon-vscode-bridge binary.
let bridge: BridgeClient | undefined;

/**
 * Called once by VS Code when the extension activates.
 * Responsible for:
 *   1. Spawning the operon-vscode-bridge sidecar process
 *   2. Registering all commands
 *   3. Registering the sidebar WebviewView provider
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  // 1. Resolve the path to the bundled bridge binary.
  //    Falls back to the user-overridden path from settings if provided.
  const configuredPath = vscode.workspace
    .getConfiguration("operon")
    .get<string>("bridgePath", "");

  const bridgeBinaryPath = configuredPath.trim() !== ""
    ? configuredPath
    : BridgeClient.defaultBinaryPath(context);

  // 2. Spawn the sidecar and connect over stdio JSON-RPC.
  bridge = new BridgeClient(bridgeBinaryPath);
  await bridge.start();

  // 3. Register the sidebar webview provider.
  //    VS Code calls createWebviewView() when the user opens the Operon sidebar.
  const chatProvider = new ChatViewProvider(context, bridge);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider("operon.chatView", chatProvider, {
      webviewOptions: { retainContextWhenHidden: true },
    })
  );

  // 4. Register commands.
  context.subscriptions.push(
    vscode.commands.registerCommand("operon.openChat", () => {
      vscode.commands.executeCommand("operon.chatView.focus");
    }),

    vscode.commands.registerCommand("operon.newSession", () => {
      chatProvider.newSession();
    }),

    vscode.commands.registerCommand("operon.cancelPrompt", () => {
      bridge?.cancel();
    })
  );

  // 5. Dispose bridge when the extension is torn down.
  context.subscriptions.push({ dispose: () => bridge?.stop() });
}

/**
 * Called by VS Code when the extension is deactivated (disabled, uninstalled,
 * or VS Code is shutting down). Clean up the bridge process.
 */
export function deactivate(): void {
  bridge?.stop();
  bridge = undefined;
}
