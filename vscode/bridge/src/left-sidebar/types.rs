//! Data Transfer Objects for the Left Sidebar.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarConversationDto {
    pub id: String,
    pub title: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarProjectDto {
    pub name: String,
    pub workspace: String,
    pub conversations: Vec<SidebarConversationDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarDataDto {
    pub chats: Vec<SidebarConversationDto>,
    pub projects: Vec<SidebarProjectDto>,
    pub active_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelContactDto {
    pub id: String,
    pub name: String,
    pub number: String,
    pub last_message: String,
    pub last_timestamp: i64,
    pub unread_count: u32,
}
