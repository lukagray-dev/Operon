'use strict';

/**
 * placeholders.js
 *
 * Comprehensive placeholder/mock data for the Operon settings panel system.
 *
 * This module provides realistic static data to replace all IPC/backend calls,
 * allowing the settings UI to work without a backend connection.
 *
 * Includes:
 *  - Model providers (OpenAI, Anthropic, Google, etc.)
 *  - Channel/connector configurations (Telegram, Discord, WhatsApp, Email)
 *  - Permission rows (global and directory-scoped)
 *  - Skills marketplace and installed skills
 *  - Extensions marketplace, downloaded, and installed extensions
 *  - About/app information
 */

// ══════════════════════════════════════════════════════════════════════════════
// MODEL PROVIDERS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * List of available model provider summary rows.
 * Shown in the Models list view as clickable provider cards.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_MODEL_PROVIDERS = [
  {
    id: 'openai',
    label: 'OpenAI',
    defaultApiBase: 'https://api.openai.com/v1',
    docsUrl: 'https://platform.openai.com/docs',
    requiresApiKey: true,
    isActive: true,
    isConfigured: true,
    activeModel: 'gpt-4o',
  },
  {
    id: 'anthropic',
    label: 'Anthropic',
    defaultApiBase: 'https://api.anthropic.com',
    docsUrl: 'https://docs.anthropic.com',
    requiresApiKey: true,
    isActive: false,
    isConfigured: true,
    activeModel: 'claude-3-5-sonnet-20241022',
  },
  {
    id: 'google',
    label: 'Google AI',
    defaultApiBase: 'https://generativelanguage.googleapis.com/v1',
    docsUrl: 'https://ai.google.dev/docs',
    requiresApiKey: true,
    isActive: false,
    isConfigured: false,
    activeModel: '',
  },
  {
    id: 'groq',
    label: 'Groq',
    defaultApiBase: 'https://api.groq.com/openai/v1',
    docsUrl: 'https://console.groq.com/docs',
    requiresApiKey: true,
    isActive: false,
    isConfigured: false,
    activeModel: '',
  },
  {
    id: 'openrouter',
    label: 'OpenRouter',
    defaultApiBase: 'https://openrouter.ai/api/v1',
    docsUrl: 'https://openrouter.ai/docs',
    requiresApiKey: true,
    isActive: false,
    isConfigured: false,
    activeModel: '',
  },
  {
    id: 'deepseek',
    label: 'DeepSeek',
    defaultApiBase: 'https://api.deepseek.com/v1',
    docsUrl: 'https://platform.deepseek.com/docs',
    requiresApiKey: true,
    isActive: false,
    isConfigured: false,
    activeModel: '',
  },
  {
    id: 'ollama',
    label: 'Ollama',
    defaultApiBase: 'http://localhost:11434',
    docsUrl: 'https://ollama.ai/docs',
    requiresApiKey: false,
    isActive: false,
    isConfigured: true,
    activeModel: 'llama2',
  },
  {
    id: 'mistral',
    label: 'Mistral AI',
    defaultApiBase: 'https://api.mistral.ai/v1',
    docsUrl: 'https://docs.mistral.ai',
    requiresApiKey: true,
    isActive: false,
    isConfigured: false,
    activeModel: '',
  },
  {
    id: 'huggingface',
    label: 'HuggingFace',
    defaultApiBase: 'https://api-inference.huggingface.co',
    docsUrl: 'https://huggingface.co/docs',
    requiresApiKey: true,
    isActive: false,
    isConfigured: false,
    activeModel: '',
  },
];

/**
 * Provider setup detail map keyed by provider ID.
 * Returned by getModelProviderSetup(providerId) placeholder.
 * @type {Map<string, Object>}
 */
export const PLACEHOLDER_PROVIDER_SETUPS = new Map([
  ['openai', {
    providerId: 'openai',
    label: 'OpenAI',
    defaultApiBase: 'https://api.openai.com/v1',
    docsUrl: 'https://platform.openai.com/docs',
    requiresApiKey: true,
    apiBase: 'https://api.openai.com/v1',
    apiKey: 'sk-placeholder-key-1234567890',
    selectedModel: 'gpt-4o',
    fallbackModels: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo'],
    isActive: true,
  }],
  ['anthropic', {
    providerId: 'anthropic',
    label: 'Anthropic',
    defaultApiBase: 'https://api.anthropic.com',
    docsUrl: 'https://docs.anthropic.com',
    requiresApiKey: true,
    apiBase: 'https://api.anthropic.com',
    apiKey: 'sk-ant-placeholder-12345',
    selectedModel: 'claude-3-5-sonnet-20241022',
    fallbackModels: ['claude-3-5-sonnet-20241022', 'claude-3-opus-20240229', 'claude-3-haiku-20240307'],
    isActive: false,
  }],
  ['google', {
    providerId: 'google',
    label: 'Google AI',
    defaultApiBase: 'https://generativelanguage.googleapis.com/v1',
    docsUrl: 'https://ai.google.dev/docs',
    requiresApiKey: true,
    apiBase: '',
    apiKey: '',
    selectedModel: '',
    fallbackModels: ['gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-pro'],
    isActive: false,
  }],
  ['groq', {
    providerId: 'groq',
    label: 'Groq',
    defaultApiBase: 'https://api.groq.com/openai/v1',
    docsUrl: 'https://console.groq.com/docs',
    requiresApiKey: true,
    apiBase: '',
    apiKey: '',
    selectedModel: '',
    fallbackModels: ['llama-3.3-70b-versatile', 'llama-3.1-8b-instant', 'mixtral-8x7b-32768'],
    isActive: false,
  }],
  ['ollama', {
    providerId: 'ollama',
    label: 'Ollama',
    defaultApiBase: 'http://localhost:11434',
    docsUrl: 'https://ollama.ai/docs',
    requiresApiKey: false,
    apiBase: 'http://localhost:11434',
    apiKey: '',
    selectedModel: 'llama2',
    fallbackModels: ['llama2', 'mistral', 'codellama', 'phi'],
    isActive: false,
  }],
]);

// ══════════════════════════════════════════════════════════════════════════════
// CHANNEL/CONNECTOR CONFIGURATIONS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * List of available connectors/channels.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_CONNECTORS = [
  {
    id: 'telegram',
    label: 'Telegram',
    enabled: true,
    externalAccessEnabled: false,
  },
  {
    id: 'discord',
    label: 'Discord',
    enabled: false,
    externalAccessEnabled: false,
  },
  {
    id: 'whatsapp',
    label: 'WhatsApp',
    enabled: false,
    externalAccessEnabled: false,
  },
  {
    id: 'email',
    label: 'Email',
    enabled: false,
    externalAccessEnabled: false,
  },
];

/**
 * Connector setup detail map keyed by connector ID.
 * @type {Map<string, Object>}
 */
export const PLACEHOLDER_CONNECTOR_SETUPS = new Map([
  ['telegram', {
    connectorId: 'telegram',
    enabled: true,
    externalAccessEnabled: false,
    allowFrom: ['123456789', '@john_doe'],
    telegramToken: '1234567890:ABCdefGHIjklMNOpqrsTUVwxyz123456789',
  }],
  ['discord', {
    connectorId: 'discord',
    enabled: false,
    externalAccessEnabled: false,
    allowFrom: [],
    discordToken: '',
  }],
  ['whatsapp', {
    connectorId: 'whatsapp',
    enabled: false,
    externalAccessEnabled: false,
    allowFrom: [],
    whatsappBridgeUrl: '',
    whatsappUseNative: true,
    whatsappSessionStorePath: '<workspace>/whatsapp-session',
  }],
  ['email', {
    connectorId: 'email',
    enabled: false,
    externalAccessEnabled: false,
    allowFrom: [],
    emailAddress: '',
    emailPassword: '',
    emailImapHost: 'imap.gmail.com',
    emailImapPort: '993',
    emailSmtpHost: 'smtp.gmail.com',
    emailSmtpPort: '587',
    emailMailbox: 'INBOX',
    emailPollIntervalSecs: '60',
    emailDisplayName: 'Operon Agent',
  }],
]);

/**
 * WhatsApp login snapshot placeholder.
 * @type {Object}
 */
export const PLACEHOLDER_WHATSAPP_LOGIN = {
  sessionStorePath: '<workspace>/whatsapp-session',
  qrText: '',
  pairCode: '',
  connected: false,
};

// ══════════════════════════════════════════════════════════════════════════════
// PERMISSIONS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Allowed directories configuration.
 * @type {Object}
 */
export const PLACEHOLDER_ALLOWED_DIRECTORIES = {
  workspaceDirectory: 'D:\\Project Operon',
  directories: ['D:\\Project Operon', 'C:\\Users\\user\\Documents'],
};

/**
 * Global permission rows (tool groups and tools).
 * @type {Array<Object>}
 */
export const PLACEHOLDER_GLOBAL_PERMISSION_ROWS = [
  // File system group
  {
    kind: 'group',
    key: 'filesystem',
    label: 'File System',
    mode: 'allow',
    baseMode: 'ask',
    isExplicit: true,
    groupKey: '',
  },
  {
    kind: 'tool',
    key: 'read_file',
    label: 'Read File',
    mode: 'allow',
    baseMode: 'allow',
    isExplicit: false,
    groupKey: 'filesystem',
  },
  {
    kind: 'tool',
    key: 'write_file',
    label: 'Write File',
    mode: 'ask',
    baseMode: 'allow',
    isExplicit: true,
    groupKey: 'filesystem',
  },
  {
    kind: 'tool',
    key: 'delete_file',
    label: 'Delete File',
    mode: 'deny',
    baseMode: 'allow',
    isExplicit: true,
    groupKey: 'filesystem',
  },
  // Shell execution group
  {
    kind: 'group',
    key: 'shell',
    label: 'Shell Execution',
    mode: 'ask',
    baseMode: 'ask',
    isExplicit: false,
    groupKey: '',
  },
  {
    kind: 'tool',
    key: 'execute_command',
    label: 'Execute Command',
    mode: 'ask',
    baseMode: 'ask',
    isExplicit: false,
    groupKey: 'shell',
  },
  {
    kind: 'tool',
    key: 'start_process',
    label: 'Start Background Process',
    mode: 'ask',
    baseMode: 'ask',
    isExplicit: false,
    groupKey: 'shell',
  },
  // Network group
  {
    kind: 'group',
    key: 'network',
    label: 'Network Access',
    mode: 'allow',
    baseMode: 'ask',
    isExplicit: true,
    groupKey: '',
  },
  {
    kind: 'tool',
    key: 'web_search',
    label: 'Web Search',
    mode: 'allow',
    baseMode: 'allow',
    isExplicit: false,
    groupKey: 'network',
  },
  {
    kind: 'tool',
    key: 'web_fetch',
    label: 'Web Fetch',
    mode: 'allow',
    baseMode: 'allow',
    isExplicit: false,
    groupKey: 'network',
  },
];

/**
 * Directory-specific permission rows.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_DIRECTORY_PERMISSION_ROWS = [
  {
    kind: 'group',
    key: 'filesystem',
    label: 'File System',
    mode: 'allow',
    baseMode: 'ask',
    isExplicit: true,
    groupKey: '',
  },
  {
    kind: 'tool',
    key: 'read_file',
    label: 'Read File',
    mode: 'allow',
    baseMode: 'allow',
    isExplicit: false,
    groupKey: 'filesystem',
  },
  {
    kind: 'tool',
    key: 'write_file',
    label: 'Write File',
    mode: 'allow',
    baseMode: 'allow',
    isExplicit: false,
    groupKey: 'filesystem',
  },
];

// ══════════════════════════════════════════════════════════════════════════════
// SKILLS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Skills search results placeholder.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_SKILLS_SEARCH_RESULTS = [
  {
    slug: 'code-review',
    displayName: 'Code Review Assistant',
    summary: 'Automated code review and best practices suggestions',
    version: '1.2.0',
    registryName: 'ohub',
  },
  {
    slug: 'security-scan',
    displayName: 'Security Scanner',
    summary: 'Detect security vulnerabilities and common attack vectors',
    version: '2.0.1',
    registryName: 'ohub',
  },
  {
    slug: 'docs-generator',
    displayName: 'Documentation Generator',
    summary: 'Generate comprehensive API and code documentation',
    version: '1.5.3',
    registryName: 'ohub',
  },
];

/**
 * Installed skills placeholder.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_INSTALLED_SKILLS = [
  {
    slug: 'code-review',
    displayName: 'Code Review Assistant',
    summary: 'Automated code review and best practices suggestions',
    version: '1.2.0',
  },
];

// ══════════════════════════════════════════════════════════════════════════════
// EXTENSIONS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Extensions search results placeholder.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_EXTENSIONS_SEARCH_RESULTS = [
  {
    slug: 'git-ops',
    displayName: 'Git Operations',
    summary: 'Git repository operations and version control tools',
    version: '1.0.0',
    registryName: 'ohub',
  },
  {
    slug: 'calendar-sync',
    displayName: 'Calendar Integration',
    summary: 'Sync and manage calendar events across platforms',
    version: '0.9.2',
    registryName: 'ohub',
  },
  {
    slug: 'sql-client',
    displayName: 'SQL Database Client',
    summary: 'Connect and query SQL databases directly',
    version: '2.1.0',
    registryName: 'ohub',
  },
];

/**
 * Downloaded extensions placeholder.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_DOWNLOADED_EXTENSIONS = [
  {
    slug: 'git-ops',
    displayName: 'Git Operations',
    summary: 'Git repository operations and version control tools',
    version: '1.0.0',
    registryName: 'ohub',
    platform: 'win32-x64',
    downloadedAt: '2024-01-15 10:30:00',
    artifactPath: '<downloads>/git-ops-1.0.0-win32-x64.tar.gz',
  },
];

/**
 * Installed extensions placeholder.
 * @type {Array<Object>}
 */
export const PLACEHOLDER_INSTALLED_EXTENSIONS = [
  {
    slug: 'git-ops',
    displayName: 'Git Operations',
    summary: 'Git repository operations and version control tools',
    version: '1.0.0',
    enabled: true,
    mcpServerName: 'git-mcp',
    authProvider: 'github',
    authConnected: true,
  },
  {
    slug: 'sql-client',
    displayName: 'SQL Database Client',
    summary: 'Connect and query SQL databases directly',
    version: '2.1.0',
    enabled: false,
    mcpServerName: 'sql-mcp',
    authProvider: '',
    authConnected: false,
  },
];

// ══════════════════════════════════════════════════════════════════════════════
// ABOUT / APP INFO
// ══════════════════════════════════════════════════════════════════════════════

/**
 * App information placeholder.
 * @type {Object}
 */
export const PLACEHOLDER_APP_INFO = {
  version: '0.1.0',
  platform: 'win32',
  arch: 'x64',
  nodeVersion: 'v20.11.0',
  tauriVersion: '1.5.4',
  buildDate: '2024-01-20',
};

// ══════════════════════════════════════════════════════════════════════════════
// ALIASES FOR BACKWARD COMPATIBILITY
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Global permissions structured by scope (owner/external).
 * Each scope contains an array of permission rows.
 * @type {Object<string, Array<Object>>}
 */
export const PLACEHOLDER_GLOBAL_PERMISSIONS = {
  owner: PLACEHOLDER_GLOBAL_PERMISSION_ROWS,
  external: PLACEHOLDER_GLOBAL_PERMISSION_ROWS.map(row => ({
    ...row,
    mode: 'deny',
    isExplicit: false,
  })),
};

/**
 * Directory-scoped permissions structured by directory path and scope.
 * Each directory has owner/external scopes with permission rows.
 * @type {Object<string, Object<string, Array<Object>>>}
 */
export const PLACEHOLDER_DIRECTORY_PERMISSIONS = {
  'D:\\Project Operon': {
    owner: PLACEHOLDER_DIRECTORY_PERMISSION_ROWS,
    external: PLACEHOLDER_DIRECTORY_PERMISSION_ROWS.map(row => ({
      ...row,
      mode: 'deny',
      isExplicit: false,
    })),
  },
  'C:\\Users\\user\\Documents': {
    owner: PLACEHOLDER_DIRECTORY_PERMISSION_ROWS,
    external: PLACEHOLDER_DIRECTORY_PERMISSION_ROWS.map(row => ({
      ...row,
      mode: 'ask',
      isExplicit: false,
    })),
  },
};
