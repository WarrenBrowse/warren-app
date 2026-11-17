import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  clearMnemonicFromClipboard,
  MNEMONIC_CLIPBOARD_LIFETIME_MS,
  MnemonicClipboard,
  MnemonicClipboardGuard,
} from '../../src/main/mnemonic-clipboard';

const MNEMONIC = 'abandon ability able about above absent absorb abstract absurd abuse access';

// Electron's main-process clipboard, as it behaves since Electron 44:
// `readText()` resolves asynchronously, like `navigator.clipboard`.
function fakeClipboard(initial: string): MnemonicClipboard & { content: () => string } {
  let text = initial;
  return {
    readText: () => Promise.resolve(text),
    clear: () => {
      text = '';
    },
    content: () => text,
  };
}

describe('clearing the mnemonic from the clipboard', () => {
  it('wipes the clipboard while it still holds the mnemonic', async () => {
    const clipboard = fakeClipboard(MNEMONIC);

    const cleared = await clearMnemonicFromClipboard(clipboard, MNEMONIC);

    expect(cleared).toBe(true);
    expect(clipboard.content()).toBe('');
  });

  it('leaves alone what the user copied in between', async () => {
    const clipboard = fakeClipboard('something else');

    const cleared = await clearMnemonicFromClipboard(clipboard, MNEMONIC);

    expect(cleared).toBe(false);
    expect(clipboard.content()).toBe('something else');
  });
});

describe('the clipboard guard armed by a copy of the mnemonic', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('wipes the mnemonic a minute later whatever became of the screen that copied it', async () => {
    vi.useFakeTimers();
    const clipboard = fakeClipboard(MNEMONIC);
    const guard = new MnemonicClipboardGuard(clipboard);

    guard.arm(MNEMONIC);
    await vi.advanceTimersByTimeAsync(MNEMONIC_CLIPBOARD_LIFETIME_MS);

    expect(clipboard.content()).toBe('');
  });

  it('keeps the mnemonic until the minute is up', async () => {
    vi.useFakeTimers();
    const clipboard = fakeClipboard(MNEMONIC);
    const guard = new MnemonicClipboardGuard(clipboard);

    guard.arm(MNEMONIC);
    await vi.advanceTimersByTimeAsync(MNEMONIC_CLIPBOARD_LIFETIME_MS - 1);

    expect(clipboard.content()).toBe(MNEMONIC);
  });

  it('wipes the mnemonic at once when the app quits', async () => {
    const clipboard = fakeClipboard(MNEMONIC);
    const guard = new MnemonicClipboardGuard(clipboard);

    guard.arm(MNEMONIC);
    await guard.flush();

    expect(clipboard.content()).toBe('');
  });
});
