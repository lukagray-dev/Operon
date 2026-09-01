# operon-channels-discord

Discord channel integration sub-crate for Operon backend.

Handles:
- Direct Discord HTTPS REST API (`/users/@me`, `/channels/{channel_id}/messages`) for bot token verification and message dispatch.
- Real-time Gateway WebSocket (`wss://gateway.discord.gg/?v=10&encoding=json`) with Opcode 10 Hello, Opcode 1 Heartbeating, Opcode 2 Identify, and Opcode 0 Dispatch (`MESSAGE_CREATE`).
- User ID allowlist role classification (`Owner` vs `External`).
- Single shared workspace root (configurable via `DiscordConfig.workspace_dir`, defaulting to `~/.operon/workspace/`).
- Per-user session history isolation (`~/.operon/sessions/discord/<user_id>/<session_id>.json`).
- `/new` session resets and turn cancellation.
- First-time onboarding documentation.
- Long-message chunking (respecting Discord's 2000 character limit).
- Live session response streaming.

