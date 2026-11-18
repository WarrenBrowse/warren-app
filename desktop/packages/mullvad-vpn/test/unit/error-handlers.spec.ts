import { EventEmitter } from 'events';
import { describe, expect, it } from 'vitest';

import {
  createRenderProcessGoneHandler,
  installProcessErrorHandlers,
} from '../../src/main/process-error-handlers';
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

// A renderer crash used to quit the whole app: the tray icon vanished and the
// person was left with a tunnel and no window to drive it. One fresh window is
// worth trying; a renderer that dies again would only crash-loop.
describe('a renderer process that dies', () => {
  function crashHandler(recreate: () => Promise<void> = () => Promise.resolve()) {
    const calls: string[] = [];
    const log = recorder();
    const handler = createRenderProcessGoneHandler({
      recreateWindow: async () => {
        calls.push('recreate');
        await recreate();
      },
      quit: () => calls.push('quit'),
      log,
    });
    return { calls, log, handler };
  }

  it('gets a new window the first time, and the app keeps running', async () => {
    const { calls, log, handler } = crashHandler();

    await handler({ reason: 'crashed', exitCode: 11 });

    expect(calls).toEqual(['recreate']);
    expect(log.lines).toEqual([expect.stringContaining('crashed')]);
  });

  it('quits the app when it dies a second time', async () => {
    const { calls, handler } = crashHandler();

    await handler({ reason: 'crashed', exitCode: 11 });
    await handler({ reason: 'oom', exitCode: 0 });

    expect(calls).toEqual(['recreate', 'quit']);
  });

  it('quits the app when the new window cannot be made', async () => {
    const { calls, handler } = crashHandler(() => Promise.reject(new Error('no display')));

    await handler({ reason: 'killed', exitCode: 9 });

    expect(calls).toEqual(['recreate', 'quit']);
  });

  it('is left alone when it exits cleanly', async () => {
    const { calls, handler } = crashHandler();

    await handler({ reason: 'clean-exit', exitCode: 0 });

    expect(calls).toEqual([]);
  });
});
