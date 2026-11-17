interface ErrorEventTarget {
  addEventListener(
    type: 'error',
    listener: (event: { error?: unknown; message?: string }) => void,
  ): void;
  addEventListener(
    type: 'unhandledrejection',
    listener: (event: { reason?: unknown }) => void,
  ): void;
}

interface ErrorLog {
  error(...parts: unknown[]): void;
}

function describe(value: unknown): string {
  return value instanceof Error ? (value.stack ?? value.message) : String(value);
}

/**
 * Logs what the renderer threw outside React rendering (an IPC callback, a
 * promise), which the error boundary never sees and which would otherwise
 * only reach the devtools console.
 */
export function installRendererErrorHandlers(target: ErrorEventTarget, log: ErrorLog) {
  target.addEventListener('error', (event: { error?: unknown; message?: string }) => {
    log.error('Uncaught error in the renderer:', describe(event.error ?? event.message));
  });
  target.addEventListener('unhandledrejection', (event: { reason?: unknown }) => {
    log.error('Unhandled promise rejection in the renderer:', describe(event.reason));
  });
}
