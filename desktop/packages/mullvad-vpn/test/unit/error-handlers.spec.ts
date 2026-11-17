import { EventEmitter } from 'events';
import { describe, expect, it } from 'vitest';

import { installProcessErrorHandlers } from '../../src/main/process-error-handlers';
import { installRendererErrorHandlers } from '../../src/renderer/lib/renderer-error-handlers';

function recorder() {
  const lines: string[] = [];
  return { lines, error: (...parts: unknown[]) => lines.push(parts.map(String).join(' ')) };
}

// An error nobody catches in the main process would otherwise open Electron's
// native error dialog, or vanish into a console nobody reads; in the renderer,
// one thrown outside React rendering never reaches the error boundary. Both
// belong in the log a problem report carries.
describe('errors nobody caught', () => {
  it('are logged by the main process, which keeps running', () => {
    const proc = new EventEmitter();
    const log = recorder();
    installProcessErrorHandlers(proc, log);

    proc.emit('uncaughtException', new Error('spawn warren-exclude ENOENT'));
    proc.emit('unhandledRejection', new Error('daemon went away'));

    expect(log.lines).toEqual([
      expect.stringContaining('spawn warren-exclude ENOENT'),
      expect.stringContaining('daemon went away'),
    ]);
  });

  it('are logged by the renderer, for errors and for rejected promises', () => {
    const target = new EventEmitter();
    const log = recorder();
    installRendererErrorHandlers(
      {
        addEventListener: (
          type: string,
          listener: (event: { error?: unknown; message?: string; reason?: unknown }) => void,
        ) => {
          target.on(type, listener);
        },
      },
      log,
    );

    target.emit('error', { error: new Error('callback failed') });
    target.emit('unhandledrejection', { reason: new Error('ipc call rejected') });

    expect(log.lines).toEqual([
      expect.stringContaining('callback failed'),
      expect.stringContaining('ipc call rejected'),
    ]);
  });
});
