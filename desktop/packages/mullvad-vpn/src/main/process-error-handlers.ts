interface ProcessEvents {
  on(event: 'uncaughtException', listener: (error: Error) => void): unknown;
  on(event: 'unhandledRejection', listener: (reason: unknown) => void): unknown;
}

interface ErrorLog {
  error(...parts: unknown[]): void;
}

/**
 * Logs what the main process threw and nobody caught, and keeps running. Left
 * alone, Electron answers an uncaught exception with its native error dialog
 * and a rejected promise with a line on a console nobody reads; neither
 * reaches the log a problem report carries. The tunnel is the daemon's, so the
 * window carrying on is the lesser harm.
 */
export function installProcessErrorHandlers(proc: ProcessEvents, log: ErrorLog) {
  proc.on('uncaughtException', (error: Error) => {
    log.error('Uncaught exception in the main process:', error?.stack ?? String(error));
  });
  proc.on('unhandledRejection', (reason: unknown) => {
    log.error(
      'Unhandled promise rejection in the main process:',
      reason instanceof Error ? (reason.stack ?? reason.message) : String(reason),
    );
  });
}
