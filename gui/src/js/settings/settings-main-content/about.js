'use strict';

/**
 * about.js
 *
 * About settings page for the Operon settings panel.
 *
 * Displays:
 *  - App logo, name, and version in a hero card.
 *  - System info grid (platform, runtime, build, Node/Tauri version).
 *  - External links (GitHub, Documentation, Report Issue).
 *
 * No async data loading is required — all content is static at render time.
 * If version info is available via ipc.getAppInfo(), it is fetched inline.
 */

import { escapeHtml } from '../settings-panel.js';
import { PLACEHOLDER_APP_INFO } from './placeholders.js';

// ── App Version Metadata ──────────────────────────────────────────────────────

/**
 * Fallback version shown when IPC is unavailable.
 * @type {Object}
 */
const FALLBACK_INFO = {
  version:      '0.1.0',
  platform:     navigator.platform || 'Unknown',
  arch:         '',
  nodeVersion:  '',
  tauriVersion: '',
  buildDate:    '',
};

// ── Page Builder ──────────────────────────────────────────────────────────────

/**
 * Builds the static HTML for the About page.
 * Uses FALLBACK_INFO immediately; the host container will be updated
 * via hydrateAboutPage() once async data is available.
 *
 * @returns {string} HTML string.
 */
function buildAboutPageContent() {
  return `
    <div class="settings-page settings-about-page">
      <h2 class="settings-page__title">About</h2>

      <!-- ── Hero Card ─────────────────────────────────────────────────── -->
      <div class="settings-about-hero">
        <div class="settings-about-hero__logo">
          <img src="./assets/icons/settings/about-operon.svg" width="44" height="44" alt="">
        </div>
        <div class="settings-about-hero__name">Operon</div>
        <div class="settings-about-hero__version" id="about-version-line">
          Version ${escapeHtml(FALLBACK_INFO.version)}
        </div>
      </div>

      <!-- ── System Info Grid ───────────────────────────────────────────── -->
      <div class="settings-about-grid" id="about-info-grid">
        ${buildInfoGrid(FALLBACK_INFO)}
      </div>

      <!-- ── External Links ─────────────────────────────────────────────── -->
      <div class="settings-about-links">
        <a href="https://github.com/operon-ai/operon" target="_blank" rel="noopener noreferrer">
          <img src="./assets/icons/settings/github.svg" width="14" height="14" alt=""> GitHub
        </a>
        <a href="https://docs.operon.ai" target="_blank" rel="noopener noreferrer">
          <img src="./assets/icons/settings/about-documentation.svg" width="14" height="14" alt=""> Documentation
        </a>
        <a href="https://github.com/operon-ai/operon/issues" target="_blank" rel="noopener noreferrer">
          <img src="./assets/icons/settings/about-report-issue.svg" width="14" height="14" alt=""> Report Issue
        </a>
      </div>
    </div>
  `;
}

// ── Async Hydration ───────────────────────────────────────────────────────────

/**
 * Fetches real app version info via IPC and updates the About page in-place.
 * Call this after injecting the page HTML.
 *
 * @param {HTMLElement} container - The settings main content element.
 */
async function hydrateAboutPage(container) {
  if (!container) return;

  try {
    // PLACEHOLDER: Use static app info data
    await new Promise(resolve => setTimeout(resolve, 300)); // Simulate async delay
    const info = PLACEHOLDER_APP_INFO;
    if (!info) return;

    const versionLine = container.querySelector('#about-version-line');
    const infoGrid    = container.querySelector('#about-info-grid');

    if (versionLine) {
      versionLine.textContent = `Version ${String(info.version || FALLBACK_INFO.version)}`;
    }

    if (infoGrid) {
      infoGrid.innerHTML = buildInfoGrid({
        version:      String(info.version      || FALLBACK_INFO.version),
        platform:     String(info.platform     || FALLBACK_INFO.platform),
        arch:         String(info.arch         || ''),
        nodeVersion:  String(info.nodeVersion  || ''),
        tauriVersion: String(info.tauriVersion || ''),
        buildDate:    String(info.buildDate    || ''),
      });
    }
  } catch {
    // Placeholder unavailable — keep fallback info; no error toast needed here.
  }
}

// ── Info Grid Builder ─────────────────────────────────────────────────────────

/**
 * Builds the key/value grid rows for system information.
 *
 * @param {{version:string, platform:string, arch:string, nodeVersion:string, tauriVersion:string, buildDate:string}} info
 * @returns {string} HTML for grid rows.
 */
function buildInfoGrid(info) {
  const rows = [
    { key: 'Version',       value: info.version      },
    { key: 'Platform',      value: info.platform      },
    { key: 'Architecture',  value: info.arch          },
    { key: 'Node.js',       value: info.nodeVersion   },
    { key: 'Tauri',         value: info.tauriVersion  },
    { key: 'Build date',    value: info.buildDate     },
  ].filter(r => r.value);

  return rows.map(r => `
    <div class="settings-about-grid__row">
      <div class="settings-about-grid__key">${escapeHtml(r.key)}</div>
      <div class="settings-about-grid__value">${escapeHtml(r.value)}</div>
    </div>
  `).join('');
}

// ── Icon Helpers ──────────────────────────────────────────────────────────────

/**
 * Returns an <img> tag for an icon from the settings icons directory.
 * Path is resolved relative to index.html using './assets/icons/settings/'.
 * @param {string} name - Icon name without extension
 * @param {number} size - Icon size in pixels
 * @returns {string} HTML img tag
 */
function icon(name, size = 14) {
  return `<img src="./assets/icons/settings/${name}.svg" width="${size}" height="${size}" alt="" draggable="false">`;
}

function buildLogoSvg() {
  return icon('about-operon', 44);
}

function githubIcon() {
  return icon('github', 14);
}

function docsIcon() {
  return icon('about-documentation', 14);
}

function bugIcon() {
  return icon('about-report-issue', 14);
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { buildAboutPageContent, hydrateAboutPage };
