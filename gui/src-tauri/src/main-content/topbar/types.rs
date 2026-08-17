//! Data Transfer Objects for the Main Content Topbar.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffStatsDto {
    pub insertions: usize,
    pub deletions: usize,
    pub files_changed: usize,
    pub is_git_repo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopbarDataDto {
    pub title: String,
    pub is_project: bool,
    pub project_name: Option<String>,
    pub git_stats: Option<GitDiffStatsDto>,
    pub unfinished_todo_count: usize,
    pub total_todo_count: usize,
}
