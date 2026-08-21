// diff.rs — Diff Parsing Engine for operon-diff
//
// Hey friend! This module provides internal helper routines for inspecting `git2::Diff` structures,
// converting low-level `libgit2` delta patches into rich high-level `FileDiff`, `DiffHunk`, and `DiffLine`
// DTOs consumable by Slint desktop UI elements.

use crate::dto::{DiffHunk, DiffLine, FileDiff};
use crate::error::DiffError;
use git2::{Diff, Repository};
use std::path::Path;

/// Helper: Parses a `git2::Diff` structure into a vector of detailed `FileDiff` structures.
///
/// Hey buddy! libgit2 provides a Patch builder API which allows us to inspect
/// files, hunks, and lines easily. We iterate over all diff deltas to retrieve
/// patch data and populate file paths, directory components, and hunk details.
pub fn parse_diff(_repo: &Repository, diff: &Diff) -> Result<Vec<FileDiff>, DiffError> {
    let mut file_diffs = Vec::new();
    let num_deltas = diff.deltas().len();

    for idx in 0..num_deltas {
        // Build a patch for each file change
        if let Ok(Some(patch)) = git2::Patch::from_diff(diff, idx) {
            let delta = patch.delta();

            // Extract the new path (or old path if it was deleted)
            let path_str = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();

            // Extract file_name and dir_path for Slint UI rendering
            let std_path = Path::new(&path_str);
            let file_name = std_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let dir_path = std_path
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();

            // Determine status label string
            let status = match delta.status() {
                git2::Delta::Added => "added",
                git2::Delta::Deleted => "deleted",
                git2::Delta::Modified => "modified",
                git2::Delta::Renamed => "renamed",
                git2::Delta::Typechange => "typechanged",
                git2::Delta::Untracked => "untracked",
                _ => "modified",
            }
            .to_string();

            // Fetch insertion and deletion line statistics for this patch
            let (insertions, deletions) = patch
                .line_stats()
                .map(|(ins, del, _)| (ins, del))
                .unwrap_or((0, 0));

            // Extract hunk modifications
            let mut hunks = Vec::new();
            let num_hunks = patch.num_hunks();

            for h_idx in 0..num_hunks {
                let (hunk, num_lines) = patch.hunk(h_idx)?;
                let header = String::from_utf8_lossy(hunk.header()).into_owned();

                let mut lines = Vec::new();
                for l_idx in 0..num_lines {
                    let line = patch.line_in_hunk(h_idx, l_idx)?;
                    let line_type = line.origin(); // Origin tells us if it's '+', '-', ' '
                    let content = String::from_utf8_lossy(line.content()).into_owned();

                    lines.push(DiffLine {
                        line_type,
                        content,
                        old_line_num: line.old_lineno(),
                        new_line_num: line.new_lineno(),
                    });
                }

                hunks.push(DiffHunk {
                    header,
                    lines,
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                });
            }

            file_diffs.push(FileDiff {
                path: path_str,
                file_name,
                dir_path,
                status,
                insertions,
                deletions,
                hunks,
                is_expanded: false,
            });
        }
    }

    Ok(file_diffs)
}
