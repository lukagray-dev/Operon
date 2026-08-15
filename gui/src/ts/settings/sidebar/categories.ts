// Settings Categories Metadata matching Slint Specification

import type { SettingsCategory } from './types.js';

export const SETTINGS_CATEGORIES: SettingsCategory[] = [
  {
    id: 'general',
    label: 'General',
    iconClass: 'icon-settings-general',
    description: 'System configuration, default parameters, and execution modes.',
  },
  {
    id: 'models',
    label: 'Models',
    iconClass: 'icon-settings-models',
    description: 'LLM providers, API keys, active model discovery, and base URLs.',
  },
  {
    id: 'appearance',
    label: 'Appearance',
    iconClass: 'icon-settings-appearance',
    description: 'User interface scaling, visual theme, and typography preferences.',
  },
  {
    id: 'channels',
    label: 'Channels',
    iconClass: 'icon-settings-channels',
    description: 'WhatsApp and Telegram companion bridge pairing and listener setup.',
  },
  {
    id: 'skills',
    label: 'Skills',
    iconClass: 'icon-settings-skills',
    description: 'Agent skills catalog, execution rules, and parameter definitions.',
  },
  {
    id: 'extensions',
    label: 'Extensions',
    iconClass: 'icon-settings-extensions',
    description: 'Model Context Protocol (MCP) servers and runtime plugin integrations.',
  },
  {
    id: 'memory',
    label: 'Memory',
    iconClass: 'icon-settings-memory',
    description: 'Vector store databases, embeddings index, and session context windows.',
  },
  {
    id: 'permissions',
    label: 'Permissions',
    iconClass: 'icon-settings-permissions',
    description: 'Sandbox policies, filesystem directory boundaries, and terminal auto-approval.',
  },
  {
    id: 'about',
    label: 'About',
    iconClass: 'icon-settings-about',
    description: 'Operon version metadata, documentation, issues, and system build info.',
  },
];
