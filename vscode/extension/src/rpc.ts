// Shared JSON-RPC types for the operon-vscode-bridge protocol.
//
// These mirror the Rust types in vscode/bridge/src/rpc.rs — any change
// here must be reflected there too.

// ── Requests (Extension → Bridge) ────────────────────────────────────────────

/** A single JSON-RPC request sent to the bridge over stdin. */
export interface RpcRequest {
  /** Unique monotonic ID for this request. */
  id: number;
  /** The method to invoke on the bridge. */
  method:
    | "submit_prompt"
    | "cancel"
    | "approve_permission"
    | "deny_permission"
    | "load_history";
  /** Method-specific parameters. */
  params: Record<string, unknown>;
}

// ── Events (Bridge → Extension) ───────────────────────────────────────────────

/** The union of all event names the bridge can emit. */
export type RpcEventName =
  | "text_delta"        // A streamed text chunk from the LLM
  | "tool_start"        // A tool call has started
  | "tool_result"       // A tool call completed
  | "tool_progress"     // Intermediate progress update from a tool
  | "permission_req"    // The agent needs user approval for a tool action
  | "token_update"      // Context window token count update
  | "agent_finished"    // The agent loop completed successfully
  | "agent_error";      // The agent loop terminated with an error

/** A single streaming event emitted by the bridge over stdout. */
export interface RpcEvent {
  /** The request ID this event belongs to. */
  id: number;
  /** The event name. */
  event: RpcEventName;
  /** Event-specific payload. */
  data: RpcEventData;
}

// ── Event payloads ────────────────────────────────────────────────────────────

export type RpcEventData =
  | TextDeltaData
  | ToolStartData
  | ToolResultData
  | ToolProgressData
  | PermissionReqData
  | TokenUpdateData
  | AgentFinishedData
  | AgentErrorData;

export interface TextDeltaData {
  /** Incremental text chunk. Append to previous chunks for the full message. */
  delta: string;
}

export interface ToolStartData {
  /** Internal tool name (e.g. "shell", "write_file"). */
  tool: string;
  /** Human-readable description of what the tool is doing. */
  label: string;
  /** Serialised tool input (JSON string). */
  input: string;
}

export interface ToolResultData {
  tool: string;
  /** Whether the tool succeeded. */
  success: boolean;
  /** Short summary of the result. */
  summary: string;
}

export interface ToolProgressData {
  tool: string;
  stage: string;
  message: string;
}

export interface PermissionReqData {
  /** Unique ID for this permission request — pass back to approve/deny. */
  permission_id: string;
  tool: string;
  /** Human-readable description of the action requiring approval. */
  description: string;
}

export interface TokenUpdateData {
  used: number;
  budget: number;
}

export interface AgentFinishedData {
  session_id: string;
}

export interface AgentErrorData {
  message: string;
}

// ── Handler type ──────────────────────────────────────────────────────────────

/** Callback type registered per-request to receive its streaming events. */
export type BridgeEventHandler = (event: RpcEvent) => void;
