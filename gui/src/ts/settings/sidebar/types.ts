// Settings Sidebar Types and Interfaces

export type SettingsTabId =
  | 'general'
  | 'models'
  | 'appearance'
  | 'channels'
  | 'skills'
  | 'extensions'
  | 'memory'
  | 'permissions'
  | 'about';

export interface SettingsCategory {
  id: SettingsTabId;
  label: string;
  iconClass: string;
  badge?: string;
  description: string;
}
