// Type definitions for Tauri global window API
declare global {
  interface Window {
    __TAURI__?: {
      core: {
        invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T>;
      };
      event: {
        listen<T>(
          event: string,
          handler: (event: { event: string; payload: T }) => void
        ): Promise<() => void>;
        emit(event: string, payload?: unknown): Promise<void>;
      };
    };
  }
}

export interface ChatMessage {
  id: string;
  sender: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: number;
}

class App {
  private messagesContainer: HTMLElement | null = null;
  private promptInput: HTMLTextAreaElement | null = null;
  private sendBtn: HTMLButtonElement | null = null;
  private statusIndicator: HTMLElement | null = null;

  constructor() {
    this.initElements();
    this.initEvents();
    this.initTauriListeners();
  }

  private initElements(): void {
    this.messagesContainer = document.getElementById('messages');
    this.promptInput = document.getElementById('prompt-input') as HTMLTextAreaElement | null;
    this.sendBtn = document.getElementById('send-btn') as HTMLButtonElement | null;
    this.statusIndicator = document.getElementById('system-status');
  }

  private initEvents(): void {
    this.sendBtn?.addEventListener('click', () => this.handleSendMessage());

    this.promptInput?.addEventListener('keydown', (e: KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        this.handleSendMessage();
      }
    });
  }

  private async initTauriListeners(): Promise<void> {
    if (!window.__TAURI__) {
      console.warn('Tauri global API not detected. Running in standard web context.');
      return;
    }

    try {
      await window.__TAURI__.event.listen<string>('agent-status', (event) => {
        if (this.statusIndicator) {
          this.statusIndicator.textContent = event.payload;
        }
      });
    } catch (err) {
      console.error('Failed to register Tauri event listeners:', err);
    }
  }

  private async handleSendMessage(): Promise<void> {
    if (!this.promptInput) return;

    const content = this.promptInput.value.trim();
    if (!content) return;

    this.addMessage({
      id: crypto.randomUUID(),
      sender: 'user',
      content,
      timestamp: Date.now(),
    });

    this.promptInput.value = '';

    if (window.__TAURI__) {
      try {
        const response = await window.__TAURI__.core.invoke<string>('send_prompt', {
          prompt: content,
        });

        this.addMessage({
          id: crypto.randomUUID(),
          sender: 'assistant',
          content: response,
          timestamp: Date.now(),
        });
      } catch (err) {
        this.addMessage({
          id: crypto.randomUUID(),
          sender: 'assistant',
          content: `Error invoking agent: ${err}`,
          timestamp: Date.now(),
        });
      }
    } else {
      // Offline / browser fallback demonstration
      setTimeout(() => {
        this.addMessage({
          id: crypto.randomUUID(),
          sender: 'assistant',
          content: `Echo: "${content}" (Tauri API not active in plain browser)`,
          timestamp: Date.now(),
        });
      }, 500);
    }
  }

  private addMessage(msg: ChatMessage): void {
    if (!this.messagesContainer) return;

    const card = document.createElement('div');
    card.className = `message-card ${msg.sender}`;
    card.textContent = msg.content;

    this.messagesContainer.appendChild(card);
    this.messagesContainer.scrollTop = this.messagesContainer.scrollHeight;
  }
}

// Bootstrap application on DOMContentLoaded
window.addEventListener('DOMContentLoaded', () => {
  new App();
});
