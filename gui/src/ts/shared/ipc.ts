// Type-safe Tauri IPC wrapper with web-context fallback

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

export async function invokeIpc<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  if (window.__TAURI__?.core) {
    try {
      return await window.__TAURI__.core.invoke<T>(cmd, args);
    } catch (err) {
      console.error(`[IPC] Error invoking ${cmd}:`, err);
      throw err;
    }
  } else {
    console.warn(`[IPC] Window.__TAURI__ unavailable. Mocked response for: ${cmd}`);
    return null;
  }
}

export async function listenIpcEvent<T = unknown>(
  event: string,
  handler: (payload: T) => void
): Promise<() => void> {
  if (window.__TAURI__?.event) {
    return await window.__TAURI__.event.listen<T>(event, (e) => {
      handler(e.payload);
    });
  }
  return () => {};
}

