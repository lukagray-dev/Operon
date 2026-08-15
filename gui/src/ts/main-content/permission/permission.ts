// Interactive Permission Dialog Controller & View Coordinator
// Replicates the Slint floating permission panel 1:1 with zero emojis and industrial design.

import { approvePermissionIpc, denyPermissionIpc } from '../messages/ipc.js';
import type { PendingPermission } from './types.js';

let currentPendingPermission: PendingPermission | null = null;

/**
 * Resolves user-friendly action verb and target name from tool and arguments.
 * Matches Slint get_permission_display_info logic.
 */
export function getPermissionDisplayInfo(
  tool: string,
  path?: string | null,
  argsJson?: string | null
): { displayAction: string; displayTarget: string } {
  let filename = '';

  if (path && path.trim().length > 0) {
    const parts = path.split(/[/\\]/);
    filename = parts[parts.length - 1] || path;
  } else if (argsJson && argsJson.trim().length > 0) {
    try {
      const val = JSON.parse(argsJson);
      const p =
        val.path ||
        val.paths ||
        val.dir ||
        val.SearchPath ||
        val.TargetFile ||
        val.AbsolutePath;
      if (typeof p === 'string' && p.trim().length > 0) {
        const parts = p.split(/[/\\]/);
        filename = parts[parts.length - 1] || p;
      } else if (typeof val.CommandLine === 'string' && val.CommandLine.trim().length > 0) {
        filename = val.CommandLine.trim();
      } else if (typeof val.command === 'string' && val.command.trim().length > 0) {
        filename = val.command.trim();
      }
    } catch {
      // Ignore JSON parse errors
    }
  }

  let displayAction: string;
  switch (tool) {
    case 'write':
    case 'edit':
    case 'append':
    case 'write_to_file':
    case 'replace_file_content':
    case 'multi_replace_file_content':
      displayAction = 'edit';
      break;
    case 'read':
    case 'read_file':
    case 'view_file':
      displayAction = 'read';
      break;
    case 'delete':
      displayAction = 'delete';
      break;
    case 'ls':
    case 'list_dir':
      displayAction = 'list files in';
      break;
    case 'grep':
    case 'grep_search':
    case 'search':
      displayAction = 'search directory';
      break;
    case 'bash':
    case 'run_command':
    case 'exec':
      displayAction = 'execute command';
      break;
    case 'web_search':
    case 'search_web':
      displayAction = 'search the web';
      break;
    case 'web_fetch':
    case 'read_url_content':
      displayAction = 'fetch web page';
      break;
    default:
      displayAction = `run ${tool}`;
      break;
  }

  const displayTarget = filename;
  return { displayAction, displayTarget };
}

/**
 * Returns currently pending permission or null.
 */
export function getPendingPermission(): PendingPermission | null {
  return currentPendingPermission;
}

/**
 * Displays the floating permission dialogue above the prompt input.
 */
export function showPermissionDialog(
  id: string,
  tool: string,
  path: string | null,
  reason: string,
  argsJson: string
): void {
  const { displayAction, displayTarget } = getPermissionDisplayInfo(tool, path, argsJson);

  currentPendingPermission = {
    id,
    tool,
    path,
    reason,
    args_json: argsJson,
    displayAction,
    displayTarget,
  };

  const container = document.getElementById('floating-permission-panel');
  if (!container) return;

  container.innerHTML = `
    <div class="permission-left-group">
      <div class="permission-lock-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect width="18" height="11" x="3" y="11" rx="2" ry="2" />
          <path d="M7 11V7a5 5 0 0 1 10 0v4" />
        </svg>
      </div>
      <div class="permission-desc-text">
        <span>Operon wants to ${escapeHtml(displayAction)}</span>
        ${displayTarget ? `<span class="perm-target-chip" id="perm-target-chip">${escapeHtml(displayTarget)}</span>` : ''}
      </div>
      <div id="perm-hover-tooltip" class="perm-hover-tooltip" style="display: none;">
        ${path ? `<div class="perm-tooltip-path">${escapeHtml(path)}</div>` : ''}
        ${reason ? `<div class="perm-tooltip-reason">Reason: ${escapeHtml(reason)}</div>` : ''}
      </div>
    </div>
    <div class="permission-actions-group">
      <button id="btn-perm-deny" class="btn-perm-deny" title="Deny tool action">Deny</button>
      <button id="btn-perm-allow" class="btn-perm-allow" title="Allow tool action">Allow</button>
    </div>
  `;

  // Wire tooltip hover events
  const targetChip = container.querySelector<HTMLElement>('#perm-target-chip');
  const tooltip = container.querySelector<HTMLElement>('#perm-hover-tooltip');
  if (targetChip && tooltip && (path || reason)) {
    targetChip.addEventListener('mouseenter', () => {
      tooltip.style.display = 'flex';
    });
    targetChip.addEventListener('mouseleave', () => {
      tooltip.style.display = 'none';
    });
  }

  // Wire Deny button
  container.querySelector('#btn-perm-deny')?.addEventListener('click', async () => {
    const permId = currentPendingPermission?.id;
    hidePermissionDialog();
    if (permId) {
      try {
        await denyPermissionIpc(permId);
      } catch (err: unknown) {
        console.error('[Permission] Failed to deny permission:', err);
      }
    }
  });

  // Wire Allow button
  container.querySelector('#btn-perm-allow')?.addEventListener('click', async () => {
    const permId = currentPendingPermission?.id;
    hidePermissionDialog();
    if (permId) {
      try {
        await approvePermissionIpc(permId);
      } catch (err: unknown) {
        console.error('[Permission] Failed to approve permission:', err);
      }
    }
  });

  container.style.display = 'flex';
}

/**
 * Hides and clears the floating permission dialogue.
 */
export function hidePermissionDialog(): void {
  currentPendingPermission = null;
  const container = document.getElementById('floating-permission-panel');
  if (container) {
    container.style.display = 'none';
    container.innerHTML = '';
  }
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
