import type { Clipboard } from 'electron';

export type MnemonicClipboard = Pick<Clipboard, 'readText' | 'clear'>;

// Wipes the clipboard only while it still holds the mnemonic, so something the
// user copied in between is never clobbered. Returns whether it was wiped.
// `readText()` is asynchronous since Electron 44: compared without `await`, a
// Promise never equals the phrase and the mnemonic silently stays behind.
export async function clearMnemonicFromClipboard(
  clipboard: MnemonicClipboard,
  mnemonic: string,
): Promise<boolean> {
  if ((await clipboard.readText()) !== mnemonic) {
    return false;
  }
  clipboard.clear();
  return true;
}
