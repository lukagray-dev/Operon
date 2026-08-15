// Models Settings TypeScript Interfaces matching Slint 1:1

export interface ProviderSummary {
  id: string;
  label: string;
  status: string;
  active_model: string;
  is_active: boolean;
}

export interface ProviderSetupDetails {
  provider_id: string;
  provider_label: string;
  api_base_url: string;
  api_key: string;
  active_model: string;
  discovered_models: string[];
}

export interface SaveProviderRequest {
  provider_id: string;
  api_base: string;
  api_key: string;
  selected_model: string;
}
