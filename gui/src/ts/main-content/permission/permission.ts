import { listenIpcEvent } from '../../shared/ipc.js';
import { sidebarState } from '../../left-sidebar/state.js';
import { approvePermissionIpc, denyPermissionIpc, getPendingPermissionsIpc, type ChannelPermissionRequest } from '../messages/ipc.js';
import type { PendingPermission } from './types.js';

let currentPendingPermission: PendingPermission | null = null;
const pendingPermissionsBySession = new Map<string, PendingPermission>();

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
  argsJson: string,
  sessionId?: string | null
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

  const targetSessionId = sessionId || sidebarState.getActiveSessionId();
  if (targetSessionId) {
    pendingPermissionsBySession.set(targetSessionId, currentPendingPermission);
    notifyPendingPermissionChange();
  }

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
    const activeSess = sidebarState.getActiveSessionId();
    if (activeSess) {
      pendingPermissionsBySession.delete(activeSess);
      notifyPendingPermissionChange();
    }
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
    const activeSess = sidebarState.getActiveSessionId();
    if (activeSess) {
      pendingPermissionsBySession.delete(activeSess);
      notifyPendingPermissionChange();
    }
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

/**
 * Checks if a specific session currently has an active pending permission request.
 */
export function hasPendingPermission(sessionId: string): boolean {
  return pendingPermissionsBySession.has(sessionId);
}

/**
 * Checks if any session in the given list currently has an active pending permission request.
 */
export function hasAnyPendingPermission(sessionIds: string[]): boolean {
  return sessionIds.some((id) => pendingPermissionsBySession.has(id));
}

type PendingPermissionListener = () => void;
const permListeners: PendingPermissionListener[] = [];

/**
 * Subscribes to changes in pending permissions across all sessions.
 */
export function onPendingPermissionsChange(listener: PendingPermissionListener): () => void {
  permListeners.push(listener);
  return () => {
    const idx = permListeners.indexOf(listener);
    if (idx !== -1) permListeners.splice(idx, 1);
  };
}

function notifyPendingPermissionChange(): void {
  permListeners.forEach((l) => {
    try {
      l();
    } catch (e) {
      console.error('[Permission] Listener notification error:', e);
    }
  });
}

/**
 * Synchronizes permission dialog display when the user switches sessions in the sidebar.
 */
export function syncPendingPermissionForActiveSession(sessionId: string | null): void {
  if (!sessionId) {
    hidePermissionDialog();
    return;
  }

  const pending = pendingPermissionsBySession.get(sessionId);
  if (pending) {
    showPermissionDialog(
      pending.id,
      pending.tool,
      pending.path,
      pending.reason,
      pending.args_json,
      sessionId
    );
  } else {
    hidePermissionDialog();
  }
}

/**
 * Initializes listeners for channel background permission requests and initial state synchronization.
 */
export async function initPermissionManager(): Promise<void> {
  // 1. Initial sync of existing pending permissions from backend
  try {
    const existing = await getPendingPermissionsIpc();
    existing.forEach((req: ChannelPermissionRequest) => {
      const { displayAction, displayTarget } = getPermissionDisplayInfo(req.tool, req.path, req.args_json);
      pendingPermissionsBySession.set(req.session_id, {
        id: req.id,
        tool: req.tool,
        path: req.path || null,
        reason: req.reason,
        args_json: req.args_json,
        displayAction,
        displayTarget,
      });
    });
    notifyPendingPermissionChange();
    syncPendingPermissionForActiveSession(sidebarState.getActiveSessionId());
  } catch (err) {
    console.warn('[Permission] Failed to fetch initial pending permissions:', err);
  }

  // 2. Listen to live channel permission requests
  await listenIpcEvent<ChannelPermissionRequest>('channel-permission-request', (req) => {
    const { displayAction, displayTarget } = getPermissionDisplayInfo(req.tool, req.path, req.args_json);
    pendingPermissionsBySession.set(req.session_id, {
      id: req.id,
      tool: req.tool,
      path: req.path || null,
      reason: req.reason,
      args_json: req.args_json,
      displayAction,
      displayTarget,
    });
    notifyPendingPermissionChange();

    if (sidebarState.getActiveSessionId() === req.session_id) {
      showPermissionDialog(req.id, req.tool, req.path || null, req.reason, req.args_json, req.session_id);
    }
  });

  // 3. Listen to live channel permission resolutions
  await listenIpcEvent<string>('channel-permission-resolved', (sessionId) => {
    pendingPermissionsBySession.delete(sessionId);
    notifyPendingPermissionChange();
    if (sidebarState.getActiveSessionId() === sessionId) {
      hidePermissionDialog();
    }
  });
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
