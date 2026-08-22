// About Settings Controller & DOM Coordinator
//
// 1:1 match with Slint about.slint:
// - Displays Hero branding card (Operon Logo, Version, Title)
// - Displays Technical Specifications table (Version, Platform, Architecture, UI Toolkit, Compiler)
// - Handles external links (GitHub, Documentation, Report Issue)

import { getAboutSystemInfoIpc, openExternalUrlIpc } from './ipc.js';
import type { AboutSystemInfo } from './types.js';

const GITHUB_REPO_URL = 'https://github.com/lukagray-dev/Operon';
const DOCS_URL = 'https://github.com/lukagray-dev/Operon#readme';
const ISSUES_URL = 'https://github.com/lukagray-dev/Operon/issues';

let systemInfo: AboutSystemInfo | null = null;

/**
 * Initializes the About settings panel.
 */
export async function initAboutSettings(): Promise<void> {
  setupExternalLinks();
  await refreshAboutInfo();
}

/**
 * Refreshes dynamic system specifications from backend.
 */
export async function refreshAboutInfo(): Promise<void> {
  try {
    systemInfo = await getAboutSystemInfoIpc();
    renderSystemInfo();
  } catch (err) {
    console.error('[AboutSettings] Failed to fetch system info:', err);
  }
}

/**
 * Renders technical specifications grid into DOM.
 */
function renderSystemInfo(): void {
  if (!systemInfo) return;

  const versionEl = document.getElementById('about-hero-version');
  const specVersion = document.getElementById('about-spec-version');
  const specPlatform = document.getElementById('about-spec-platform');
  const specArch = document.getElementById('about-spec-arch');
  const specToolkit = document.getElementById('about-spec-toolkit');
  const specCompiler = document.getElementById('about-spec-compiler');

  if (versionEl) versionEl.textContent = `Version ${systemInfo.version}`;
  if (specVersion) specVersion.textContent = systemInfo.version;
  if (specPlatform) specPlatform.textContent = systemInfo.platform;
  if (specArch) specArch.textContent = systemInfo.architecture;
  if (specToolkit) specToolkit.textContent = systemInfo.ui_toolkit;
  if (specCompiler) specCompiler.textContent = systemInfo.compiler;
}

/**
 * Sets up click event listeners for GitHub, Documentation, and Report Issue buttons.
 */
function setupExternalLinks(): void {
  document.getElementById('btn-about-github')?.addEventListener('click', async () => {
    await openExternalUrlIpc(GITHUB_REPO_URL);
  });

  document.getElementById('btn-about-docs')?.addEventListener('click', async () => {
    await openExternalUrlIpc(DOCS_URL);
  });

  document.getElementById('btn-about-issues')?.addEventListener('click', async () => {
    await openExternalUrlIpc(ISSUES_URL);
  });
}
