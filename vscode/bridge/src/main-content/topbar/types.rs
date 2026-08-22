//! Data Transfer Objects for the Main Content Topbar in Bridge.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopbarDataDto {
    pub title: String,
    pub is_project: bool,
    pub project_name: Option<String>,
    pub unfinished_todo_count: usize,
    pub total_todo_count: usize,
}
