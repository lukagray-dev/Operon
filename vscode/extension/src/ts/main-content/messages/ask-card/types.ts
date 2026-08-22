// TypeScript interfaces for Interactive Ask Question / Clarification Prompt Cards in VS Code

export interface AskQuestionData {
  id: string;
  question: string;
  options: string[];
  answer?: string;
  is_answered: boolean;
}
