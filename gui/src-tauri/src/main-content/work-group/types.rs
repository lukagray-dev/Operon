//! Data Transfer Objects for Assistant WorkGroup & Tool Cards.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum WorkGroupItemDto {
    #[serde(rename = "thinking")]
    Thinking {
        thinking_text: String,
        is_expanded: bool,
    },
    #[serde(rename = "tool")]
    Tool {
        call_id: String,
        tool_name: String,
        tool_title: String,
        tool_args: String,
        tool_result: String,
        tool_status: String,
        is_expanded: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkGroupDto {
    pub items: Vec<WorkGroupItemDto>,
    pub is_active: bool,
    pub is_expanded: bool,
    pub elapsed_secs: i64,
}
