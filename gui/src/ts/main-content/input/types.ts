// TypeScript interfaces for Main Content Input Panel

export interface PendingAttachment {
  path: string;
  file_name: string;
  is_image: boolean;
  size_bytes: number;
}

export interface ModelOption {
  id: string;
  name: string;
  is_active: boolean;
  context_window: number;
}

export interface ContextUsage {
  tokens_used: number;
  tokens_total: number;
  percentage: number;
  formatted: string;
}

export type ReasoningLevel = 'Low' | 'Medium' | 'High' | 'Disabled';
