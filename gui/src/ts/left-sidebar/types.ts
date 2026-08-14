// TypeScript interfaces for Left Sidebar

export interface SidebarConversation {
  id: string;
  title: string;
  created_at: number;
}

export interface SidebarProject {
  name: string;
  workspace: string;
  conversations: SidebarConversation[];
}

export interface SidebarData {
  chats: SidebarConversation[];
  projects: SidebarProject[];
  active_session_id?: string | null;
}

export interface ChannelContact {
  id: string;
  name: string;
  number: string;
  last_message: string;
  last_timestamp: number;
  unread_count: number;
}
