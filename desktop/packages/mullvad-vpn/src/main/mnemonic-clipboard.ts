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

// Long enough to paste the phrase into a password manager, short enough that
// it does not sit in the clipboard (or sync through its history) for good.
export const MNEMONIC_CLIPBOARD_LIFETIME_MS = 60_000;

/**
 * Wipes a copied mnemonic from the clipboard once its lifetime is up, or when
 * the app quits. It lives in the main process so the wipe does not depend on
 * the screen that copied the phrase: a renderer timer dies with its component,
 * and the backup screens are left within seconds of the copy.
 */
export class MnemonicClipboardGuard {
  private pending?: { mnemonic: string; timer: ReturnType<typeof setTimeout> };

  public constructor(private readonly clipboard: MnemonicClipboard) {}

  public arm(mnemonic: string): void {
    this.cancel();
    const timer = setTimeout(() => void this.flush(), MNEMONIC_CLIPBOARD_LIFETIME_MS);
    this.pending = { mnemonic, timer };
  }

  /** Wipes now, if a copy is pending. Returns whether the clipboard was wiped. */
  public flush(): Promise<boolean> {
    const pending = this.pending;
    this.cancel();
    return pending
      ? clearMnemonicFromClipboard(this.clipboard, pending.mnemonic)
      : Promise.resolve(false);
  }

  private cancel(): void {
    if (this.pending) {
      clearTimeout(this.pending.timer);
      this.pending = undefined;
    }
  }
}
