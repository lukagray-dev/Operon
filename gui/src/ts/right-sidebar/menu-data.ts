// Context Menu and Submenu Definitions for Source Control Right Sidebar

import type { ContextMenuItem } from './types.js';

// 1. Primary Source Control Header Overflow Context Menu
export const overflowMenu: ContextMenuItem[] = [
  { id: 'view_sub', label: 'View', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'sep1', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'commit_sub', label: 'Commit', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'changes_sub', label: 'Changes', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'pull_push_sub', label: 'Pull, Push', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'branch_sub', label: 'Branch', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'remote_sub', label: 'Remote', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'stash_sub', label: 'Stash', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'tags_sub', label: 'Tags', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'worktrees_sub', label: 'Worktrees', shortcut: '', has_submenu: true, is_separator: false, is_disabled: false },
  { id: 'sep2', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'cmd_show_output', label: 'Show Git Output', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_reopen_closed_editor', label: 'Reopen Closed Editor', shortcut: 'Ctrl+Shift+T', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_clear_editor_history', label: 'Clear Editor History', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 2. View Submenu Items
export const viewSubmenu: ContextMenuItem[] = [
  { id: 'toggle_repos', label: '✓ Repositories', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'toggle_changes', label: '✓ Source Control (Changes)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'toggle_graph', label: '✓ Source Control (Commit Graph)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep1', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'view_tree_mode', label: 'View as Tree', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'view_list_mode', label: 'View as List', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 3. Commit Submenu Items
export const commitSubmenu: ContextMenuItem[] = [
  { id: 'cmd_commit', label: 'Commit', shortcut: 'Ctrl+Enter', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_commit_staged', label: 'Commit Staged', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_commit_all', label: 'Commit All', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep1', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'cmd_commit_amend', label: 'Commit (Amend)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_commit_staged_amend', label: 'Commit Staged (Amend)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_commit_all_amend', label: 'Commit All (Amend)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep2', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'cmd_commit_signed_off', label: 'Commit (Signed Off)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_commit_staged_signed_off', label: 'Commit Staged (Signed Off)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_commit_all_signed_off', label: 'Commit All (Signed Off)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 4. Changes Submenu Items
export const changesSubmenu: ContextMenuItem[] = [
  { id: 'cmd_stage_all', label: 'Stage All Changes', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_unstage_all', label: 'Unstage All Changes', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_discard_all', label: 'Discard All Changes', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 5. Pull/Push Submenu Items
export const pullPushSubmenu: ContextMenuItem[] = [
  { id: 'cmd_sync', label: 'Sync', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_pull', label: 'Pull', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_pull_rebase', label: 'Pull (Rebase)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_pull_from', label: 'Pull from...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep1', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'cmd_push', label: 'Push', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_push_to', label: 'Push to...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep2', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'cmd_fetch', label: 'Fetch', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_fetch_prune', label: 'Fetch (Prune)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_fetch_all_remotes', label: 'Fetch From All Remotes', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 6. Branch Submenu Items
export const branchSubmenu: ContextMenuItem[] = [
  { id: 'cmd_merge', label: 'Merge...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_rebase_branch', label: 'Rebase Branch...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_create_branch', label: 'Create Branch...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_create_branch_from', label: 'Create Branch From...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_rename_branch', label: 'Rename Branch...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_delete_branch', label: 'Delete Branch...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_delete_remote_branch', label: 'Delete Remote Branch...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_publish_branch', label: 'Publish Branch...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 7. Remote Submenu Items
export const remoteSubmenu: ContextMenuItem[] = [
  { id: 'cmd_add_remote', label: 'Add Remote...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_remove_remote', label: 'Remove Remote', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 8. Stash Submenu Items
export const stashSubmenu: ContextMenuItem[] = [
  { id: 'cmd_stash', label: 'Stash', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_stash_untracked', label: 'Stash (Include Untracked)', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_stash_staged', label: 'Stash Staged', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep1', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'cmd_apply_latest_stash', label: 'Apply Latest Stash', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_apply_stash', label: 'Apply Stash...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_pop_latest_stash', label: 'Pop Latest Stash', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_pop_stash', label: 'Pop Stash...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep2', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'cmd_drop_stash', label: 'Drop Stash...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_drop_all_stashes', label: 'Drop All Stashes...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_view_stash', label: 'View Stash...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 9. Tags Submenu Items
export const tagsSubmenu: ContextMenuItem[] = [
  { id: 'cmd_create_tag', label: 'Create Tag...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_delete_tag', label: 'Delete Tag...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_delete_remote_tag', label: 'Delete Remote Tag...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'cmd_push_tags', label: 'Push Tags', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 10. Worktrees Submenu Items
export const worktreesSubmenu: ContextMenuItem[] = [
  { id: 'cmd_create_worktree', label: 'Create Worktree...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];

// 11. Commit Graph Heading Menu
export const graphHeadingMenu: ContextMenuItem[] = [
  { id: 'graph_refresh', label: 'Refresh Graph', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'graph_show_remote', label: 'Show Remote Branches', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'graph_center_head', label: 'Center HEAD Commit', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'graph_filter_branch', label: 'Filter by Current Branch', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
  { id: 'sep1', label: '', shortcut: '', has_submenu: false, is_separator: true, is_disabled: false },
  { id: 'graph_settings', label: 'Graph Settings...', shortcut: '', has_submenu: false, is_separator: false, is_disabled: false },
];
