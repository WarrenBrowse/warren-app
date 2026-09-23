import fs from 'fs';

import log from '../shared/logging';
import { PendingPurchaseStore } from './purchase-flow';
import { SecretStore } from './torrent-client/password-store';

/**
 * The sealed blob holding the purchases still waiting for their voucher,
 * beside the other sealed blobs under the app's user-data directory.
 */
export const PENDING_PURCHASES_FILE = 'purchases.bin';

/**
 * Where the pending purchases wait for their voucher.
 *
 * Each entry carries the purchase's pull secret, the only thing that collects
 * the voucher, so it never goes into `gui_settings.json`, which is cleartext
 * by invariant. With a platform keychain the entries are sealed in a file of
 * their own that only its owner can read, so a purchase paid after the app was
 * closed is still collected on the next run. The seal keeps them out of
 * backups and of anything that reads the file; on Windows and Linux the
 * keychain still opens it for any program of the same user, as it does for the
 * other sealed blobs. Without a keychain (Linux with no keyring) the entries
 * stay in memory for the life of the process and are never written. The file
 * goes with the last entry, which the purchase flow drops once its voucher is
 * collected or the server has let it lapse.
 */
export class SealedPendingPurchaseStore implements PendingPurchaseStore {
  private entries: string[] = [];
  private loaded = false;

  public constructor(
    private readonly secrets: SecretStore,
    private readonly filePath: () => string,
  ) {}

  public get(): string[] {
    if (!this.loaded) {
      this.entries = this.load();
      this.loaded = true;
    }
    return [...this.entries];
  }

  public set(entries: string[]): void {
    this.entries = [...entries];
    this.loaded = true;
    // A blob this run cannot open (a keyring locked for the session) was
    // never read, so this run does not know it is finished with: it stays.
    if (!this.secrets.available) {
      return;
    }
    try {
      if (entries.length === 0) {
        fs.rmSync(this.filePath(), { force: true });
      } else {
        fs.writeFileSync(this.filePath(), this.secrets.encrypt(JSON.stringify(entries)), {
          mode: 0o600,
        });
      }
    } catch (e) {
      log.error(`Failed to store the pending purchases: ${(e as Error).message}`);
    }
  }

  private load(): string[] {
    if (!this.secrets.available || !fs.existsSync(this.filePath())) {
      return [];
    }
    try {
      const entries: unknown = JSON.parse(this.secrets.decrypt(fs.readFileSync(this.filePath())));
      if (Array.isArray(entries) && entries.every((entry) => typeof entry === 'string')) {
        return entries;
      }
    } catch {
      // The error stays out of the log: a parse error quotes the text it
      // choked on, and that text holds the pull secrets.
    }
    log.warn('Failed to read the pending purchases');
    return [];
  }
}
