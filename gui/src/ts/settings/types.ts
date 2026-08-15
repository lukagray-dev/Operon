// Settings Data Transfer Objects and Frontend Interfaces

export type { SettingsCategory, SettingsTabId } from './sidebar/types.js';

export interface SettingsState {
  activeTab: import('./sidebar/types.js').SettingsTabId;
}
