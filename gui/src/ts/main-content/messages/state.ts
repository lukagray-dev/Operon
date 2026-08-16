// Messages Local State Manager
//
// Optimized for 60fps streaming with fine-grained callbacks and chronological
// interleaved WorkGroup and Text blocks.

import { getToolFriendlyTitle } from '../work-group/work-group.js';
import type { ChatMessage, MessageBlock } from './types.js';

type MessagesChangeListener = () => void;
type StreamTextListener = (messageId: string, blockIndex: number, fullBlockText: string, chunk: string) => void;
type StreamWorkGroupListener = (messageId: string, blockIndex: number) => void;
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

  public truncateAndStartTurn(turnIndex: number, newPromptText: string): void {
    this.stopStreamingTimer();
    this.streamingMessageId = null;

    // 1. Keep only messages strictly prior to this turn
    this.messages = this.messages.filter((m) => m.turn_index < turnIndex);

    // 2. Add the updated user message for this turn
    const userMsg: ChatMessage = {
      id: `turn_${turnIndex}_user`,
      role: 'user',
      text: newPromptText,
      timestamp: 'Just now',
      created_at: Math.floor(Date.now() / 1000),
      turn_index: turnIndex,
      is_liked: false,
      is_disliked: false,
    };
    this.messages.push(userMsg);

    // 3. Prepare streaming assistant placeholder
    this.startAssistantStreaming(turnIndex);

    // 4. Trigger full reset / rerender of the messages view
    for (const l of this.fullResetListeners) {
      l();
    }
    this.notify();
  }

  public startAssistantStreaming(turnIndex: number): string {
    this.stopStreamingTimer();
    const id = `turn_${turnIndex}_assistant`;
    this.streamingStartTime = Date.now();

    const initialWorkGroup = {
      items: [],
      is_active: true,
      is_expanded: false,
      elapsed_secs: 0,
    };

    const msg: ChatMessage = {
      id,
      role: 'assistant',
      text: '',
      timestamp: 'Just now',
      created_at: Math.floor(Date.now() / 1000),
      turn_index: turnIndex,
      is_liked: false,
      is_disliked: false,
      work_group: initialWorkGroup,
      blocks: [
        {
          kind: 'work_group',
          data: initialWorkGroup,
        },
      ],
    };

    this.messages.push(msg);
    this.streamingMessageId = id;

    // Start interval to update elapsed seconds every second while working
    this.streamingTimerInterval = window.setInterval(() => {
      if (this.streamingMessageId && this.streamingStartTime) {
        const target = this.messages.find((m) => m.id === this.streamingMessageId);
        if (target && target.blocks) {
          let updated = false;
          target.blocks.forEach((block, bIdx) => {
            if (block.kind === 'work_group' && block.data.is_active) {
              block.data.elapsed_secs = Math.max(
                1,
                Math.floor((Date.now() - (this.streamingStartTime || Date.now())) / 1000)
              );
              for (const l of this.streamWorkGroupListeners) {
                l(this.streamingMessageId!, bIdx);
              }
              updated = true;
            }
          });
          if (updated && target.work_group && target.work_group.is_active) {
            target.work_group.elapsed_secs = Math.max(
              1,
              Math.floor((Date.now() - (this.streamingStartTime || Date.now())) / 1000)
            );
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
    if (!msg) return;

    if (!msg.blocks) {
      msg.blocks = [];
    }

    const isWhitespaceOnly = text.trim().length === 0;
    let lastBlock = msg.blocks[msg.blocks.length - 1];

    // If text is pure whitespace and the active block is a WorkGroup, do not start a text block
    if (isWhitespaceOnly && (!lastBlock || lastBlock.kind === 'work_group')) {
      return;
    }

    // If the last block is a WorkGroup, conclude its active state and create a new text block
    if (!lastBlock || lastBlock.kind === 'work_group') {
      if (lastBlock && lastBlock.kind === 'work_group') {
        lastBlock.data.is_active = false;
      }
      const newTextBlock: MessageBlock = { kind: 'text', text: '' };
      msg.blocks.push(newTextBlock);
      lastBlock = newTextBlock;
    }

    // Append to current text block
    if (lastBlock.kind === 'text') {
      if (lastBlock.text.length === 0) {
        text = text.replace(/^[\r\n]+/, '');
      }
      if (text.length === 0) return;

      lastBlock.text += text;
      const blockIdx = msg.blocks.length - 1;

      // Update consolidated text
      msg.text = msg.blocks
        .filter((b): b is { kind: 'text'; text: string } => b.kind === 'text')
        .map((b) => b.text)
        .join('\n\n');

      for (const l of this.streamTextListeners) {
        l(this.streamingMessageId, blockIdx, lastBlock.text, text);
      }
    }
  }

  public appendThinkingDelta(text: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (!msg) return;

    if (!msg.blocks) {
      msg.blocks = [];
    }

    let lastBlock = msg.blocks[msg.blocks.length - 1];

    // If last block is a text block, trim its trailing whitespace, update its DOM text, then start a new WorkGroup block chronologically after it
    if (!lastBlock || lastBlock.kind === 'text') {
      if (lastBlock && lastBlock.kind === 'text') {
        lastBlock.text = lastBlock.text.trimEnd();
        const prevTextIdx = msg.blocks.length - 1;
        for (const l of this.streamTextListeners) {
          l(this.streamingMessageId, prevTextIdx, lastBlock.text, '');
        }
      }
      const newWgBlock: MessageBlock = {
        kind: 'work_group',
        data: {
          items: [],
          is_active: true,
          is_expanded: false,
          elapsed_secs: 0,
        },
      };
      msg.blocks.push(newWgBlock);
      lastBlock = newWgBlock;
    }

    const blockIdx = msg.blocks.length - 1;
    const workGroup = lastBlock.data;

    // Chronological thinking: append to last item if it's thinking, otherwise start new thinking item
    const lastItem = workGroup.items[workGroup.items.length - 1];
    if (lastItem && lastItem.kind === 'thinking') {
      lastItem.thinking_text += text;
    } else {
      workGroup.items.push({
        kind: 'thinking',
        thinking_text: text,
        is_expanded: false,
      });
    }

    for (const l of this.streamWorkGroupListeners) {
      l(this.streamingMessageId, blockIdx);
    }
  }

  public addToolCallStart(callId: string, name: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (!msg) return;

    if (!msg.blocks) {
      msg.blocks = [];
    }

    let lastBlock = msg.blocks[msg.blocks.length - 1];

    // If last block is a text block, trim its trailing whitespace, update its DOM text, then start a new WorkGroup block chronologically after it
    if (!lastBlock || lastBlock.kind === 'text') {
      if (lastBlock && lastBlock.kind === 'text') {
        lastBlock.text = lastBlock.text.trimEnd();
        const prevTextIdx = msg.blocks.length - 1;
        for (const l of this.streamTextListeners) {
          l(this.streamingMessageId, prevTextIdx, lastBlock.text, '');
        }
      }
      const newWgBlock: MessageBlock = {
        kind: 'work_group',
        data: {
          items: [],
          is_active: true,
          is_expanded: false,
          elapsed_secs: 0,
        },
      };
      msg.blocks.push(newWgBlock);
      lastBlock = newWgBlock;
    }

    const blockIdx = msg.blocks.length - 1;
    const workGroup = lastBlock.data;

    const existing = workGroup.items.find((i) => i.kind === 'tool' && i.call_id === callId);
    if (!existing) {
      workGroup.items.push({
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
        l(this.streamingMessageId, blockIdx);
      }
    }
  }

  public setToolCallArgs(callId: string, argsJson: string): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (!msg || !msg.blocks) return;

    msg.blocks.forEach((block, bIdx) => {
      if (block.kind === 'work_group') {
        const tool = block.data.items.find((i) => i.kind === 'tool' && i.call_id === callId);
        if (tool && tool.kind === 'tool') {
          tool.tool_args = argsJson;
          tool.tool_title = getToolFriendlyTitle(tool.tool_name, argsJson);
          for (const l of this.streamWorkGroupListeners) {
            l(this.streamingMessageId!, bIdx);
          }
        }
      }
    });
  }

  public setToolCallResult(callId: string, result: string, isError: boolean): void {
    if (!this.streamingMessageId) return;
    const msg = this.messages.find((m) => m.id === this.streamingMessageId);
    if (!msg || !msg.blocks) return;

    msg.blocks.forEach((block, bIdx) => {
      if (block.kind === 'work_group') {
        const tool = block.data.items.find((i) => i.kind === 'tool' && i.call_id === callId);
        if (tool && tool.kind === 'tool') {
          tool.tool_result = result;
          tool.tool_status = isError ? 'failed' : 'completed';
          for (const l of this.streamWorkGroupListeners) {
            l(this.streamingMessageId!, bIdx);
          }
        }
      }
    });
  }

  public toggleWorkGroupExpanded(messageId: string, blockIndex = 0): void {
    const msg = this.messages.find((m) => m.id === messageId);
    if (msg) {
      if (msg.blocks && msg.blocks[blockIndex] && msg.blocks[blockIndex].kind === 'work_group') {
        const wg = (msg.blocks[blockIndex] as { kind: 'work_group'; data: import('../work-group/types.js').WorkGroupData }).data;
        wg.is_expanded = !wg.is_expanded;
      } else if (msg.work_group) {
        msg.work_group.is_expanded = !msg.work_group.is_expanded;
      }
      for (const l of this.streamWorkGroupListeners) {
        l(messageId, blockIndex);
      }
    }
  }

  public toggleWorkGroupItemExpanded(messageId: string, blockIndex: number, itemIdx: number): void {
    const msg = this.messages.find((m) => m.id === messageId);
    if (msg) {
      if (msg.blocks && msg.blocks[blockIndex] && msg.blocks[blockIndex].kind === 'work_group') {
        const wg = (msg.blocks[blockIndex] as { kind: 'work_group'; data: import('../work-group/types.js').WorkGroupData }).data;
        if (wg.items[itemIdx]) {
          wg.items[itemIdx].is_expanded = !wg.items[itemIdx].is_expanded;
        }
      } else if (msg.work_group && msg.work_group.items[itemIdx]) {
        msg.work_group.items[itemIdx].is_expanded = !msg.work_group.items[itemIdx].is_expanded;
      }
      for (const l of this.streamWorkGroupListeners) {
        l(messageId, blockIndex);
      }
    }
  }

  public finishStreaming(): void {
    if (this.streamingMessageId) {
      const id = this.streamingMessageId;
      const msg = this.messages.find((m) => m.id === id);
      if (msg) {
        if (msg.blocks) {
          msg.blocks.forEach((block) => {
            if (block.kind === 'work_group') {
              block.data.is_active = false;
              if (this.streamingStartTime) {
                block.data.elapsed_secs = Math.max(
                  1,
                  Math.floor((Date.now() - this.streamingStartTime) / 1000)
                );
              }
            }
          });
        }
        if (msg.work_group) {
          msg.work_group.is_active = false;
          if (this.streamingStartTime) {
            msg.work_group.elapsed_secs = Math.max(
              1,
              Math.floor((Date.now() - this.streamingStartTime) / 1000)
            );
          }
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
    for (const l of this.fullResetListeners) {
      l();
    }
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
