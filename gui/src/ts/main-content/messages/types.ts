// TypeScript interfaces for Chat Messages

import type { WorkGroupData } from '../work-group/types.js';
import type { AskQuestionData } from './ask-card/types.js';

export type MessageBlock =
  | { kind: 'work_group'; data: WorkGroupData }
  | { kind: 'text'; text: string }
  | { kind: 'ask'; data: AskQuestionData };

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  text: string;
  timestamp: string;
  created_at: number;
  turn_index: number;
  is_liked: boolean;
  is_disliked: boolean;
  work_group?: WorkGroupData;
  blocks?: MessageBlock[];
}
