import type { BrowserWindow, Rectangle } from 'electron';
import { afterEach, describe, expect, it, vi } from 'vitest';

import WindowController from '../../src/main/window-controller';

vi.mock('electron', () => ({
  screen: {
    addListener: () => undefined,
    removeListener: () => undefined,
    getPrimaryDisplay: () => ({
      workArea: { x: 0, y: 0, width: 1920, height: 1080 },
      size: { width: 1920, height: 1080 },
    }),
  },
  ipcMain: { on: () => undefined, handle: () => undefined },
}));

const realPlatform = process.platform;

function runOn(platform: NodeJS.Platform) {
  Object.defineProperty(process, 'platform', { value: platform });
}

afterEach(() => {
  Object.defineProperty(process, 'platform', { value: realPlatform });
});

describe('the desktop window', () => {
  it('opens at the size the home screen is laid out for', () => {
    runOn('darwin');

    expect(WindowController.getContentSize(false)).toEqual({ width: 380, height: 620 });
  });

  // Since Electron 41 an unpinned window on Windows gets the height it is
  // given, and a window pinned to the taskbar comes out 8px short of it
  // (electron/electron#50783).
  it('asks Windows for the exact height when unpinned', () => {
    runOn('win32');

    expect(WindowController.getContentSize(true)).toEqual({ width: 380, height: 620 });
  });

  it('asks Windows for 8px more when pinned to the taskbar', () => {
    runOn('win32');

    expect(WindowController.getContentSize(false)).toEqual({ width: 380, height: 628 });
  });

  // Since Electron 41 `setBounds` sizes the whole window, decorations
  // included, on a Linux compositor that draws client-side decorations
  // (electron/electron#49209), which would shrink the page below the size the
  // home screen is laid out for.
  it('sizes the page, not the frame, when it repositions the window', () => {
    runOn('linux');
    const contentBounds: Rectangle = { x: 100, y: 200, width: 380, height: 620 };
    const setBounds = vi.fn();
    const setContentBounds = vi.fn();
    const window = {
      webContents: { isDestroyed: () => false },
      isDestroyed: () => false,
      once: () => undefined,
      addListener: () => undefined,
      // The frame of a CSD window is larger than its page.
      getBounds: () => ({ x: 90, y: 160, width: 400, height: 670 }),
      getContentBounds: () => contentBounds,
      setBounds,
      setContentBounds,
    } as unknown as BrowserWindow;
    const controller = new WindowController(
      { isUnpinnedWindow: () => true, getTrayBounds: () => contentBounds },
      window,
    );

    controller.updatePosition();

    expect(setBounds).not.toHaveBeenCalled();
    expect(setContentBounds).toHaveBeenCalledWith(
      { x: 100, y: 200, width: 380, height: 620 },
      false,
    );
  });
});
