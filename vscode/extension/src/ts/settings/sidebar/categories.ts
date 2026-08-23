// Settings Categories Metadata matching Slint Specification (1:1 with sidebar.slint)

import type { SettingsCategory } from './types.js';

export const SETTINGS_CATEGORIES: SettingsCategory[] = [
  {
    id: 'general',
    label: 'General',
    iconClass: 'icon-settings-general',
    description: 'System configuration, default parameters, and execution modes.',
  },
  {
    id: 'appearance',
    label: 'Appearance',
    iconClass: 'icon-settings-appearance',
    description: 'User interface scaling, visual theme, and typography preferences.',
  },
  {
    id: 'models',
    label: 'Models',
    iconClass: 'icon-settings-models',
    description: 'LLM providers, API keys, active model discovery, and base URLs.',
  },
  {
    id: 'memory',
    label: 'Memory',
    iconClass: 'icon-settings-memory',
    description: 'Inspect, add, edit, and delete agent long-term memories stored in SQLite.',
  },
  {
    id: 'permissions',
    label: 'Permissions',
    iconClass: 'icon-settings-permissions',
    description: 'Sandbox policies, filesystem directory boundaries, and tool execution policies.',
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
    id: 'about',
    label: 'About',
    iconClass: 'icon-settings-about',
    description: 'Operon version metadata, documentation, issues, and system build info.',
  },
];
