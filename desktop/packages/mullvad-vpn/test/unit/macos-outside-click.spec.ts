import { EventEmitter } from 'events';
import { describe, expect, it, vi } from 'vitest';

import { installOutsideClickHide } from '../../src/main/macos-outside-click';

// The forwarder is a native module, so it can fail to load on a machine whose
// dyld refuses the binary. Beta 1.1.60 awaited it before the tray click
// handlers were installed: the rejection skipped them and the window load, and
// the menu bar icon stayed on screen doing nothing when clicked.
describe('installOutsideClickHide', () => {
  it('starts the forwarder when the window shows and stops it when it hides', async () => {
    const window = new EventEmitter();
    const hide = vi.fn();
    const stop = vi.fn(() => true);
    const start = vi.fn((_cb: () => void) => stop);

    const installed = await installOutsideClickHide(window, hide, () => Promise.resolve({ start }));

    expect(installed).toBe(true);
    window.emit('show');
    expect(start).toHaveBeenCalledTimes(1);
    start.mock.calls[0][0]();
    expect(hide).toHaveBeenCalledTimes(1);
    window.emit('hide');
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it('resolves without the forwarder when the native module cannot be loaded', async () => {
    const window = new EventEmitter();

    const installed = await installOutsideClickHide(window, vi.fn(), () =>
      Promise.reject(new Error('mis-aligned LINKEDIT string pool')),
    );

    expect(installed).toBe(false);
    expect(window.listenerCount('show')).toBe(0);
  });

  it('keeps the show event harmless when the forwarder fails to start', async () => {
    const window = new EventEmitter();
    const start = vi.fn(() => {
      throw new Error('event tap refused');
    });

    await installOutsideClickHide(window, vi.fn(), () => Promise.resolve({ start }));

    expect(() => window.emit('show')).not.toThrow();
    expect(() => window.emit('hide')).not.toThrow();
  });
});
