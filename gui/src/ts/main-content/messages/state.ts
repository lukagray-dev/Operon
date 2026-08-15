// Messages Local State Manager
//
// Optimized for 60fps streaming with fine-grained callbacks to avoid
// rebuilding the entire DOM tree on every token or work-group delta.

import { getToolFriendlyTitle } from '../work-group/work-group.js';
import type { ChatMessage } from './types.js';

type MessagesChangeListener = () => void;
type StreamTextListener = (messageId: string, fullText: string, chunk: string) => void;
type StreamWorkGroupListener = (messageId: string) => void;
type StreamFinishedListener = (messageId: string) => void;

class MessagesStateManager {
  private messages: ChatMessage[] = [];
  private isLoading = false;
  private streamingMessageId: string | null = null;
  private streamingStartTime: number | null = null;
  private streamingTimerInterval: number | null = null;
  private listeners: Set<MessagesChangeListener> = new Set();
  private streamTextListeners: Set<StreamTextListener> = new Set();
  private streamWorkGroupListeners: Set<StreamWorkGroupListener> = new Set();
  private streamFinishedListeners: Set<StreamFinishedListener> = new Set();

  private fullResetListeners: Set<() => void> = new Set();

  public getMessages(): ChatMessage[] {
    return this.messages;
  }

  public getStreamingMessageId(): string | null {
    return this.streamingMessageId;
  }

  public getMessageById(id: string): ChatMessage | undefined {
    return this.messages.find((m) => m.id === id);
  }

  public setMessages(messages: ChatMessage[]): void {
    this.stopStreamingTimer();
    this.messages = messages;
    this.streamingMessageId = null;
    for (const l of this.fullResetListeners) {
      l();
    }
    this.notify();
  }

  public addMessage(msg: ChatMessage): void {
    this.messages.push(msg);
    this.notify();
  }

  public startAssistantStreaming(turnIndex: number): string {
    this.stopStreamingTimer();
    const id = `turn_${turnIndex}_assistant`;
    this.streamingStartTime = Date.now();

    const msg: ChatMessage = {
      id,
      role: 'assistant',
      text: '',
      timestamp: 'Just now',
      created_at: Math.floor(Date.now() / 1000),
      turn_index: turnIndex,
      is_liked: false,
      is_disliked: false,
      work_group: {
        items: [],
        is_active: true,
        is_expanded: false,
        elapsed_secs: 0,
      },
    };

    this.messages.push(msg);
    this.streamingMessageId = id;

    // Start interval to update elapsed seconds every second while working
    this.streamingTimerInterval = window.setInterval(() => {
      if (this.streamingMessageId && this.streamingStartTime) {
        const target = this.messages.find((m) => m.id === this.streamingMessageId);
        if (target && target.work_group && target.work_group.is_active) {
          target.work_group.elapsed_secs = Math.max(
            1,
            Math.floor((Date.now() - this.streamingStartTime) / 1000)
          );
          for (const l of this.streamWorkGroupListeners) {
            l(this.streamingMessageId);
          }
        }
      }
    }, 1000);

    this.notify();
    return id;
  }

  public appendStreamText(text: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (msg) {
      msg.text += text;
      for (const l of this.streamTextListeners) {
        l(this.streamingMessageId, msg.text, text);
      }
    }
  }

  public appendThinkingDelta(text: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (msg) {
      if (!msg.work_group) {
        msg.work_group = { items: [], is_active: true, is_expanded: false, elapsed_secs: 0 };
      }
      const thinkingItem = msg.work_group.items.find((i) => i.kind === 'thinking');
      if (!thinkingItem) {
        msg.work_group.items.push({
          kind: 'thinking',
          thinking_text: text,
          is_expanded: false,
        });
      } else if (thinkingItem.kind === 'thinking') {
        thinkingItem.thinking_text += text;
      }
      for (const l of this.streamWorkGroupListeners) {
        l(this.streamingMessageId);
      }
    }
  }

  public addToolCallStart(callId: string, name: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (msg) {
      if (!msg.work_group) {
        msg.work_group = { items: [], is_active: true, is_expanded: false, elapsed_secs: 0 };
      }
      const existing = msg.work_group.items.find((i) => i.kind === 'tool' && i.call_id === callId);
      if (!existing) {
        msg.work_group.items.push({
          kind: 'tool',
          call_id: callId,
          tool_name: name,
          tool_title: getToolFriendlyTitle(name, ''),
          tool_args: '',
          tool_result: '',
          tool_status: 'running',
          is_expanded: false,
        });
        for (const l of this.streamWorkGroupListeners) {
          l(this.streamingMessageId);
        }
      }
    }
  }

  public setToolCallArgs(callId: string, argsJson: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (msg && msg.work_group) {
      const tool = msg.work_group.items.find((i) => i.kind === 'tool' && i.call_id === callId);
      if (tool && tool.kind === 'tool') {
        tool.tool_args = argsJson;
        tool.tool_title = getToolFriendlyTitle(tool.tool_name, argsJson);
        for (const l of this.streamWorkGroupListeners) {
          l(this.streamingMessageId);
        }
      }
    }
  }

  public setToolCallResult(callId: string, result: string, isError: boolean): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (msg && msg.work_group) {
      const tool = msg.work_group.items.find((i) => i.kind === 'tool' && i.call_id === callId);
      if (tool && tool.kind === 'tool') {
        tool.tool_result = result;
        tool.tool_status = isError ? 'failed' : 'completed';
        for (const l of this.streamWorkGroupListeners) {
          l(this.streamingMessageId);
        }
      }
    }
  }

  public toggleWorkGroupExpanded(messageId: string): void {
    const msg = this.messages.find((m) => m.id === messageId);
    if (msg && msg.work_group) {
      msg.work_group.is_expanded = !msg.work_group.is_expanded;
      for (const l of this.streamWorkGroupListeners) {
        l(messageId);
      }
    }
  }

  public toggleWorkGroupItemExpanded(messageId: string, itemIdx: number): void {
    const msg = this.messages.find((m) => m.id === messageId);
    if (msg && msg.work_group && msg.work_group.items[itemIdx]) {
      msg.work_group.items[itemIdx].is_expanded = !msg.work_group.items[itemIdx].is_expanded;
      for (const l of this.streamWorkGroupListeners) {
        l(messageId);
      }
    }
  }

  public finishStreaming(): void {
    if (this.streamingMessageId) {
      const id = this.streamingMessageId;
      const msg = this.messages.find((m) => m.id === id);
      if (msg && msg.work_group) {
        msg.work_group.is_active = false;
        if (this.streamingStartTime) {
          msg.work_group.elapsed_secs = Math.max(
            1,
            Math.floor((Date.now() - this.streamingStartTime) / 1000)
          );
        }
      }
      this.stopStreamingTimer();
      this.streamingMessageId = null;
      for (const l of this.streamFinishedListeners) {
        l(id);
      }
      this.notify();
    }
  }

  public clear(): void {
    this.stopStreamingTimer();
    this.messages = [];
    this.streamingMessageId = null;
    this.notify();
  }

  private stopStreamingTimer(): void {
    if (this.streamingTimerInterval !== null) {
      clearInterval(this.streamingTimerInterval);
      this.streamingTimerInterval = null;
    }
    this.streamingStartTime = null;
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

  public onStreamText(listener: StreamTextListener): () => void {
    this.streamTextListeners.add(listener);
    return () => this.streamTextListeners.delete(listener);
  }

  public onStreamWorkGroup(listener: StreamWorkGroupListener): () => void {
    this.streamWorkGroupListeners.add(listener);
    return () => this.streamWorkGroupListeners.delete(listener);
  }

  public onStreamFinished(listener: StreamFinishedListener): () => void {
    this.streamFinishedListeners.add(listener);
    return () => this.streamFinishedListeners.delete(listener);
  }

  public onFullReset(listener: () => void): () => void {
    this.fullResetListeners.add(listener);
    return () => this.fullResetListeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const messagesState = new MessagesStateManager();
