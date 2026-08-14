// TypeScript interfaces for Chat Messages

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  text: string;
  timestamp: string;
  created_at: number;
  turn_index: number;
  is_liked: boolean;
  is_disliked: boolean;
}
