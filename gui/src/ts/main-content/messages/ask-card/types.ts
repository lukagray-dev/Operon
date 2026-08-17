// TypeScript interfaces for Interactive Ask Question / Clarification Prompt Cards
// Matches Rust AskQuestionDto & SessionEvent::AskQuestion

export interface AskQuestionData {
  /** Unique tool call identifier matching the ask question event */
  id: string;
  /** The question asked by the model */
  question: string;
  /** The 3 multiple-choice options provided by the model */
  options: string[];
  /** The user's chosen or typed answer (if answered) */
  answer?: string;
  /** Whether this question prompt has already been answered */
  is_answered: boolean;
}
