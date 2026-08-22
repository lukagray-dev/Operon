// Central application state manager for the frontend

type StateChangeListener = () => void;

class AppStateManager {
    private sidebarOpen = true;
    private isMaximized = false;
    private activeMenu: string | null = null;
    private listeners: Set<StateChangeListener> = new Set();

    public getSidebarOpen(): boolean {
        return this.sidebarOpen;
    }

    public setSidebarOpen(open: boolean): void {
        if (this.sidebarOpen !== open) {
            this.sidebarOpen = open;
            this.notify();
        }
    }

    public toggleSidebar(): boolean {
        this.sidebarOpen = !this.sidebarOpen;
        this.notify();
        return this.sidebarOpen;
    }

    public getIsMaximized(): boolean {
        return this.isMaximized;
    }

    public setIsMaximized(max: boolean): void {
        if (this.isMaximized !== max) {
            this.isMaximized = max;
            this.notify();
        }
    }

    public getActiveMenu(): string | null {
        return this.activeMenu;
    }

    public setActiveMenu(menu: string | null): void {
        if (this.activeMenu !== menu) {
            this.activeMenu = menu;
            this.notify();
        }
    }

    public subscribe(listener: StateChangeListener): () => void {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }

    private notify(): void {
        for (const listener of this.listeners) {
            listener();
        }
    }
}

export const appState = new AppStateManager();
