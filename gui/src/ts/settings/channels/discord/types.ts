// Discord Settings TypeScript Types

export interface DiscordState {
  connection_status: string;
  bot_token: string;
  owner_user_id: string;
  allowlist: string[];
  guild_id: string;
  workspace_dir: string;
  resolved_workspace_placeholder: string;
  is_policy_covered: boolean;
}

export interface SaveDiscordPayload {
  bot_token: string;
  owner_user_id: string;
  allowlist: string[];
  guild_id: string;
  workspace_dir: string;
}

