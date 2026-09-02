# operon-channels-feishu

Feishu / Lark messaging channel integration crate for the Operon autonomous AI workspace.

## Architecture

1. **Authentication & Domains**:
   - Supports **Feishu** (`https://open.feishu.cn`) and **Lark** (`https://open.larksuite.com`).
   - Obtains and caches `tenant_access_token` automatically via `POST /open-apis/auth/v3/tenant_access_token/internal`.
   - Tests bot credentials via `GET /open-apis/bot/v3/info`.
2. **WebSocket Long Connection & Web APIs**:
   - Receives events via WebSocket persistent connection (`wss://ws-open.feishu.cn/ws/v2` / `wss://ws-open.larksuite.com/ws/v2`).
   - Posts messages and replies via `POST /open-apis/im/v1/messages`.
3. **Session & Security Model**:
   - Routes sessions to `~/.operon/sessions/feishu/<user_id>/<session_id>.json` using `fs-<hex_timestamp>` format.
   - Enforces sequential turn execution per user via asynchronous mutex locks.
   - Checks workspace directory policy coverage with warnings for unpermitted paths.
   - Follows the WhatsApp/Discord/Slack permission pattern with desktop GUI approval management.

