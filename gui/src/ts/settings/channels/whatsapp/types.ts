// WhatsApp Settings TypeScript Types

export interface WhatsAppState {
  connection_status: string;
  owner_number: string;
  allowlist: string[];
  workspace_dir: string;
  resolved_workspace_placeholder: string;
  is_policy_covered: boolean;
}

export interface SaveWhatsAppPayload {
  owner_number: string;
  allowlist: string[];
  workspace_dir: string;
}
