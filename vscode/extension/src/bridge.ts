// BridgeClient — manages the operon-vscode-bridge child process and the
// JSON-RPC protocol over its stdin/stdout pipes.
//
// Protocol overview (newline-delimited JSON):
//   Extension → Bridge:  { "id": number, "method": string, "params": object }
//   Bridge → Extension:  { "id": number, "event": string, "data": object }
//
// The bridge emits multiple event messages per request (streaming), terminated
// by an "agent_finished" or "agent_error" event for that id.

import * as cp from "child_process";
import * as path from "path";
import * as os from "os";
import * as vscode from "vscode";
import { RpcRequest, RpcEvent, BridgeEventHandler } from "./rpc";

export class BridgeClient {
  // The spawned child process.
  private proc: cp.ChildProcess | undefined;

  // Monotonically increasing request counter.
  private nextId = 1;

  // Map of request id → registered event handler callback.
  private handlers = new Map<number, BridgeEventHandler>();

  // Buffer for partial lines received from stdout (handles split reads).
  private lineBuffer = "";

  constructor(private readonly binaryPath: string) {}

  // ── Lifecycle ─────────────────────────────────────────────────────────────

  /**
   * Spawns the bridge binary and wires up stdout/stderr listeners.
   * Throws if the binary cannot be found or fails to start.
   */
  async start(): Promise<void> {
    this.proc = cp.spawn(this.binaryPath, [], {
      stdio: ["pipe", "pipe", "pipe"],
      // Detach false: bridge dies with the extension host process
      detached: false,
    });

    // Stream stderr to the VS Code output channel for debugging
    const outputChannel = vscode.window.createOutputChannel("Operon Bridge");

    this.proc.stderr?.on("data", (chunk: Buffer) => {
      outputChannel.appendLine(`[bridge] ${chunk.toString().trim()}`);
    });

    // Route each complete stdout line through the event dispatcher
    this.proc.stdout?.on("data", (chunk: Buffer) => {
      this.lineBuffer += chunk.toString();
      const lines = this.lineBuffer.split("\n");
      // Keep the last (potentially incomplete) fragment in the buffer
      this.lineBuffer = lines.pop() ?? "";
      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed) {
          this.dispatchLine(trimmed);
        }
      }
    });

    this.proc.on("exit", (code) => {
      outputChannel.appendLine(`[bridge] exited with code ${code}`);
    });
  }

  /** Sends a SIGTERM to the bridge and cleans up internal state. */
  stop(): void {
    this.proc?.kill("SIGTERM");
    this.proc = undefined;
    this.handlers.clear();
  }

  // ── Request API ───────────────────────────────────────────────────────────

  /**
   * Submits a user prompt to the agent.
   * The handler is called for every streaming event until agent_finished/agent_error.
   */
  submitPrompt(
    sessionId: string | undefined,
    prompt: string,
    workspacePath: string | undefined,
    handler: BridgeEventHandler
  ): number {
    const id = this.nextId++;
    this.handlers.set(id, handler);
    this.send({
      id,
      method: "submit_prompt",
      params: { session_id: sessionId, prompt, workspace_path: workspacePath },
    });
    return id;
  }

  /** Cancels any in-flight prompt. */
  cancel(): void {
    this.send({ id: this.nextId++, method: "cancel", params: {} });
  }

  /** Approves a pending tool permission request. */
  approvePermission(permissionId: string): void {
    this.send({
      id: this.nextId++,
      method: "approve_permission",
      params: { permission_id: permissionId },
    });
  }

  /** Denies a pending tool permission request. */
  denyPermission(permissionId: string): void {
    this.send({
      id: this.nextId++,
      method: "deny_permission",
      params: { permission_id: permissionId },
    });
  }

  // ── Internals ─────────────────────────────────────────────────────────────

  /** Serialises a request and writes it to the bridge's stdin. */
  private send(req: RpcRequest): void {
    if (!this.proc?.stdin) {
      vscode.window.showErrorMessage("Operon: bridge is not running.");
      return;
    }
    this.proc.stdin.write(JSON.stringify(req) + "\n");
  }

  /**
   * Parses one line of stdout as an RpcEvent and routes it to the correct
   * handler. Terminal events (agent_finished, agent_error) clean up the handler.
   */
  private dispatchLine(line: string): void {
    let event: RpcEvent;
    try {
      event = JSON.parse(line) as RpcEvent;
    } catch {
      // Non-JSON output from bridge — ignore (may be startup logs)
      return;
    }

    const handler = this.handlers.get(event.id);
    if (!handler) return;

    handler(event);

    // Remove handler once the stream is complete
    if (event.event === "agent_finished" || event.event === "agent_error") {
      this.handlers.delete(event.id);
    }
  }

  // ── Static helpers ────────────────────────────────────────────────────────

  /**
   * Returns the path to the bundled bridge binary for the current platform.
   * The binary is expected at: extension/bin/operon-vscode-bridge[.exe]
   */
  static defaultBinaryPath(context: vscode.ExtensionContext): string {
    const ext = os.platform() === "win32" ? ".exe" : "";
    return path.join(
      context.extensionPath,
      "bin",
      `operon-vscode-bridge${ext}`
    );
  }
}
