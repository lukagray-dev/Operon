// Left Sidebar local reactive state

import type { ChannelContact, SidebarConversation, SidebarData, SidebarProject } from './types.js';

type SidebarChangeListener = () => void;

class SidebarStateManager {
  private chats: SidebarConversation[] = [];
  private projects: SidebarProject[] = [];
  private whatsappContacts: ChannelContact[] = [];
  private telegramContacts: ChannelContact[] = [];
  private activeSessionId: string | null = null;
  private activeProjectPath: string | null = null;
  private searchQuery = '';
  private projectsCollapsed = false;
  private chatsCollapsed = false;
  private whatsappCollapsed = false;
  private telegramCollapsed = false;
  private listeners: Set<SidebarChangeListener> = new Set();

  public getChats(): SidebarConversation[] {
    return this.chats;
  }

  public getProjects(): SidebarProject[] {
    return this.projects;
  }

  public getWhatsAppContacts(): ChannelContact[] {
    return this.whatsappContacts;
  }

  public getTelegramContacts(): ChannelContact[] {
    return this.telegramContacts;
  }

  public getActiveSessionId(): string | null {
    return this.activeSessionId;
  }

  public setActiveSessionId(id: string | null): void {
    if (this.activeSessionId !== id) {
      this.activeSessionId = id;
      this.notify();
    }
  }

  public getActiveProjectPath(): string | null {
    return this.activeProjectPath;
  }

  public setActiveProjectPath(path: string | null): void {
    if (this.activeProjectPath !== path) {
      this.activeProjectPath = path;
      this.notify();
    }
  }

  public getSearchQuery(): string {
    return this.searchQuery;
  }

  public setSearchQuery(query: string): void {
    if (this.searchQuery !== query) {
      this.searchQuery = query;
      this.notify();
    }
  }

  public isProjectsCollapsed(): boolean {
    return this.projectsCollapsed;
  }

  public toggleProjectsCollapsed(): void {
    this.projectsCollapsed = !this.projectsCollapsed;
    this.notify();
  }

  public isChatsCollapsed(): boolean {
    return this.chatsCollapsed;
  }

  public toggleChatsCollapsed(): void {
    this.chatsCollapsed = !this.chatsCollapsed;
    this.notify();
  }

  public isWhatsAppCollapsed(): boolean {
    return this.whatsappCollapsed;
  }

  public toggleWhatsAppCollapsed(): void {
    this.whatsappCollapsed = !this.whatsappCollapsed;
    this.notify();
  }

  public isTelegramCollapsed(): boolean {
    return this.telegramCollapsed;
  }

  public toggleTelegramCollapsed(): void {
    this.telegramCollapsed = !this.telegramCollapsed;
    this.notify();
  }

  public setSidebarData(data: SidebarData): void {
    this.chats = data.chats;
    this.projects = data.projects;
    if (data.active_session_id) {
      this.activeSessionId = data.active_session_id;
    }
    this.notify();
  }

  public setChannelContacts(whatsapp: ChannelContact[], telegram: ChannelContact[]): void {
    this.whatsappContacts = whatsapp;
    this.telegramContacts = telegram;
    this.notify();
  }

  public subscribe(listener: SidebarChangeListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const sidebarState = new SidebarStateManager();
