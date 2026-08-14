// Messages Local State Manager

import type { ChatMessage } from './types.js';

type MessagesChangeListener = () => void;

class MessagesStateManager {
  private messages: ChatMessage[] = [];
  private isLoading = false;
  private streamingMessageId: string | null = null;
  private listeners: Set<MessagesChangeListener> = new Set();

  public getMessages(): ChatMessage[] {
    return this.messages;
  }

  public setMessages(messages: ChatMessage[]): void {
    this.messages = messages;
    this.streamingMessageId = null;
    this.notify();
  }

  public addMessage(msg: ChatMessage): void {
    this.messages.push(msg);
    this.notify();
  }

  public startAssistantStreaming(turnIndex: number): string {
    const id = `stream_${turnIndex}_${Date.now()}`;
    const msg: ChatMessage = {
      id,
      role: 'assistant',
      text: '',
      timestamp: 'Just now',
      created_at: Math.floor(Date.now() / 1000),
      turn_index: turnIndex,
      is_liked: false,
      is_disliked: false,
    };
    this.messages.push(msg);
    this.streamingMessageId = id;
    this.notify();
    return id;
  }

  public appendStreamText(text: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (msg) {
      msg.text += text;
      this.notify();
    }
  }

  public finishStreaming(): void {
    this.streamingMessageId = null;
  }

  public clear(): void {
    this.messages = [];
    this.streamingMessageId = null;
    this.notify();
  }

  public getIsLoading(): boolean {
    return this.isLoading;
  }

  public setIsLoading(loading: boolean): void {
    if (this.isLoading !== loading) {
      this.isLoading = loading;
      this.notify();
    }
  }

  public toggleLike(messageId: string): void {
    const msg = this.messages.find((m) => m.id === messageId);
    if (msg) {
      msg.is_liked = !msg.is_liked;
      if (msg.is_liked) msg.is_disliked = false;
      this.notify();
    }
  }

  public toggleDislike(messageId: string): void {
    const msg = this.messages.find((m) => m.id === messageId);
    if (msg) {
      msg.is_disliked = !msg.is_disliked;
      if (msg.is_disliked) msg.is_liked = false;
      this.notify();
    }
  }

  public subscribe(listener: MessagesChangeListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const messagesState = new MessagesStateManager();
