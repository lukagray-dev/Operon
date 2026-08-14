class App {
    messagesContainer = null;
    promptInput = null;
    sendBtn = null;
    statusIndicator = null;
    constructor() {
        this.initElements();
        this.initEvents();
        this.initTauriListeners();
    }
    initElements() {
        this.messagesContainer = document.getElementById('messages');
        this.promptInput = document.getElementById('prompt-input');
        this.sendBtn = document.getElementById('send-btn');
        this.statusIndicator = document.getElementById('system-status');
    }
    initEvents() {
        this.sendBtn?.addEventListener('click', () => this.handleSendMessage());
        this.promptInput?.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                this.handleSendMessage();
            }
        });
    }
    async initTauriListeners() {
        if (!window.__TAURI__) {
            console.warn('Tauri global API not detected. Running in standard web context.');
            return;
        }
        try {
            await window.__TAURI__.event.listen('agent-status', (event) => {
                if (this.statusIndicator) {
                    this.statusIndicator.textContent = event.payload;
                }
            });
        }
        catch (err) {
            console.error('Failed to register Tauri event listeners:', err);
        }
    }
    async handleSendMessage() {
        if (!this.promptInput)
            return;
        const content = this.promptInput.value.trim();
        if (!content)
            return;
        this.addMessage({
            id: crypto.randomUUID(),
            sender: 'user',
            content,
            timestamp: Date.now(),
        });
        this.promptInput.value = '';
        if (window.__TAURI__) {
            try {
                const response = await window.__TAURI__.core.invoke('send_prompt', {
                    prompt: content,
                });
                this.addMessage({
                    id: crypto.randomUUID(),
                    sender: 'assistant',
                    content: response,
                    timestamp: Date.now(),
                });
            }
            catch (err) {
                this.addMessage({
                    id: crypto.randomUUID(),
                    sender: 'assistant',
                    content: `Error invoking agent: ${err}`,
                    timestamp: Date.now(),
                });
            }
        }
        else {
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
    addMessage(msg) {
        if (!this.messagesContainer)
            return;
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
export {};
//# sourceMappingURL=main.js.map