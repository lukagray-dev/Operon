// TypeScript interfaces for Assistant WorkGroup & Tool Cards for VS Code

export interface WorkGroupToolItem {
  kind: 'tool';
  call_id: string;
  tool_name: string;
  tool_title: string;
  tool_args: string;
  tool_result: string;
  tool_status: 'running' | 'completed' | 'failed';
  is_expanded: boolean;
}

export interface WorkGroupThinkingItem {
  kind: 'thinking';
  thinking_text: string;
  is_expanded: boolean;
}

export type WorkGroupItem = WorkGroupToolItem | WorkGroupThinkingItem;

export interface WorkGroupData {
  items: WorkGroupItem[];
  is_active: boolean;
  is_expanded: boolean;
  elapsed_secs: number;
}
