// Main Content Input Panel local state

import type { ContextUsage, ModelOption, PendingAttachment, ReasoningLevel } from './types.js';

type InputChangeListener = () => void;

class InputStateManager {
  private inputText = '';
  private pendingAttachments: PendingAttachment[] = [];
  private autoApproveEnabled = false;
  private selectedModel = '';
  private availableModels: ModelOption[] = [];
  private selectedReasoning: ReasoningLevel = 'Medium';
  private isVoiceRecording = false;
  private isResponding = false;
  private isReadOnly = false;
  private readOnlyReason = '';
  private contextUsage: ContextUsage = {
    tokens_used: 0,
    tokens_total: 0,
    percentage: 0,
    formatted: '',
  };
  private listeners: Set<InputChangeListener> = new Set();

  public getInputText(): string {
    return this.inputText;
  }

  public setInputText(text: string): void {
    if (this.inputText !== text) {
      this.inputText = text;
      this.notify();
    }
  }

  public getPendingAttachments(): PendingAttachment[] {
    return this.pendingAttachments;
  }

  public addAttachments(attachments: PendingAttachment[]): void {
    this.pendingAttachments.push(...attachments);
    this.notify();
  }

  public removeAttachment(index: number): void {
    if (index >= 0 && index < this.pendingAttachments.length) {
      this.pendingAttachments.splice(index, 1);
      this.notify();
    }
  }

  public clearAttachments(): void {
    this.pendingAttachments = [];
    this.notify();
  }

  public isAutoApproveEnabled(): boolean {
    return this.autoApproveEnabled;
  }

  public setAutoApproveEnabled(enabled: boolean): void {
    if (this.autoApproveEnabled !== enabled) {
      this.autoApproveEnabled = enabled;
      this.notify();
    }
  }

  public toggleAutoApprove(): boolean {
    this.autoApproveEnabled = !this.autoApproveEnabled;
    this.notify();
    return this.autoApproveEnabled;
  }

  public getSelectedModel(): string {
    return this.selectedModel;
  }

  public setSelectedModel(model: string): void {
    if (this.selectedModel !== model) {
      this.selectedModel = model;
      this.notify();
    }
  }

  public getAvailableModels(): ModelOption[] {
    return this.availableModels;
  }

  public getActiveModelOption(): ModelOption | undefined {
    return this.availableModels.find((m) => m.id === this.selectedModel) || this.availableModels.find((m) => m.is_active);
  }

  public setAvailableModels(models: ModelOption[]): void {
    this.availableModels = models;
    const active = models.find((m) => m.is_active);
    if (active) {
      this.selectedModel = active.id;
      if (active.selected_reasoning) {
        this.selectedReasoning = active.selected_reasoning;
      } else if (active.reasoning_levels && active.reasoning_levels.length > 0) {
        this.selectedReasoning = active.reasoning_levels[0];
      } else {
        this.selectedReasoning = '';
      }

      if (active.context_window) {
        const total = active.context_window;
        const totalStr = total >= 1_000_000 ? `${total / 1_000_000}M` : `${Math.round(total / 1_000)}k`;
        const used = this.contextUsage.tokens_used;
        const usedStr =
          used >= 1_000_000
            ? `${(used / 1_000_000).toFixed(1)}M`
            : used >= 1000
            ? `${(used / 1000).toFixed(1)}k`
            : `${used}`;
        this.contextUsage = {
          tokens_used: used,
          tokens_total: total,
          percentage: total > 0 ? (used / total) * 100 : 0,
          formatted: `${usedStr} / ${totalStr}`,
        };
      }
    }
    this.notify();
  }

  public getSelectedReasoning(): ReasoningLevel {
    return this.selectedReasoning;
  }

  public setSelectedReasoning(level: ReasoningLevel): void {
    if (this.selectedReasoning !== level) {
      this.selectedReasoning = level;
      this.notify();
    }
  }

  public getIsVoiceRecording(): boolean {
    return this.isVoiceRecording;
  }

  public setIsVoiceRecording(recording: boolean): void {
    if (this.isVoiceRecording !== recording) {
      this.isVoiceRecording = recording;
      this.notify();
    }
  }

  public getIsResponding(): boolean {
    return this.isResponding;
  }

  public setIsResponding(responding: boolean): void {
    if (this.isResponding !== responding) {
      this.isResponding = responding;
      this.notify();
    }
  }

  public getIsReadOnly(): boolean {
    return this.isReadOnly;
  }

  public getReadOnlyReason(): string {
    return this.readOnlyReason;
  }

  public setReadOnly(readOnly: boolean, reason = ''): void {
    if (this.isReadOnly !== readOnly || this.readOnlyReason !== reason) {
      this.isReadOnly = readOnly;
      this.readOnlyReason = reason;
      this.notify();
    }
  }

  public getContextUsage(): ContextUsage {
    return this.contextUsage;
  }

  public setContextUsage(usage: ContextUsage, force = false): void {
    if (!force && usage.tokens_used === 0 && this.contextUsage.tokens_used > 0 && usage.tokens_total === this.contextUsage.tokens_total) {
      return;
    }
    this.contextUsage = usage;
    this.notify();
  }

  public subscribe(listener: InputChangeListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const inputState = new InputStateManager();
