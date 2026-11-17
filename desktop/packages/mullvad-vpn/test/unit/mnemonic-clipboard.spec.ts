import { describe, expect, it } from 'vitest';

import { clearMnemonicFromClipboard, MnemonicClipboard } from '../../src/main/mnemonic-clipboard';

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
