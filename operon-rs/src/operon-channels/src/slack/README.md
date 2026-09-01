# operon-channels-slack

Slack channel integration crate for Operon.

## Architecture

This crate provides full Slack bidirectional communication without requiring public webhooks or reverse proxies by utilizing **Slack Socket Mode** over WebSockets alongside standard Slack REST APIs.

### Key Components

1. **`SlackClient`**:
   - Authenticates and validates tokens via `POST https://slack.com/api/auth.test`.
   - Opens WebSocket tunnel via `POST https://slack.com/api/apps.connections.open` with App-Level Token (`xapp-...`).
   - Streams events over WebSocket, immediately acknowledging each envelope with `{"envelope_id": "..."}`.
   - Dispatches outbound responses via `POST https://slack.com/api/chat.postMessage` using Bot User OAuth Token (`xoxb-...`).

2. **`SlackRouter`**:
   - Classifies senders as `Owner` (matching configured Owner User ID or Allowlist) or `External`.
   - Handles `/new` session resets.
   - Generates session IDs with `sl-<hex_timestamp>`.

3. **`SlackWorkspaceManager`**:
   - Isolates session history under `~/.operon/sessions/slack/<user_id>/<session_id>.json`.
   - Shares default or custom workspace directory (`~/.operon/workspace`).

4. **`SessionRunnerBridge`**:
   - Restores full conversation history from `SessionStore` JSON files before running turns.
   - Streams events and chunks long responses (Slack limit: 4000 chars) while preserving markdown code fences.

5. **`SlackService`**:
   - Enforces sequential turn processing per user using mutex locks (`user_locks`).
   - Manages automatic offline buffering and flushing.

