// Telegram Settings TypeScript Types

export interface TelegramState {
  connection_status: string;
  bot_token: string;
  owner_chat_id: string;
  allowlist: string[];
  workspace_dir: string;
  resolved_workspace_placeholder: string;
  is_policy_covered: boolean;
}

export interface SaveTelegramPayload {
  bot_token: string;
  owner_chat_id: string;
  allowlist: string[];
  workspace_dir: string;
}
