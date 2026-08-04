import { app, safeStorage } from 'electron';
import fs from 'fs';
import path from 'path';

import log from '../shared/logging';
import { RenewalState, RenewalStore } from './renewal-flow';

// OS-keychain-encrypted file store for the renewal mandate. The bearer
// token must NEVER land in gui_settings.json (identity-free, cleartext
// by invariant), so it gets its own blob under userData, sealed with
// electron safeStorage (Keychain / DPAPI / kwallet-libsecret). When the
// OS provides no real encryption, available() is false and the flow
// refuses to adopt a mandate at all: fail closed over cleartext.

const FILE_NAME = 'renewal.bin';

export default class SafeStorageRenewalStore implements RenewalStore {
  private cache?: RenewalState;
  private loaded = false;

  public available(): boolean {
    try {
      if (!safeStorage.isEncryptionAvailable()) {
        return false;
      }
      // Linux without a live keyring falls back to the basic_text
      // backend, which "seals" with a hardcoded key while still
      // reporting encryption as available: that is cleartext-equivalent
      // for a bearer charge authorization, so refuse it (fail closed).
      return (
        process.platform !== 'linux' || safeStorage.getSelectedStorageBackend() !== 'basic_text'
      );
    } catch {
      return false;
    }
  }

  public get(): RenewalState | undefined {
    if (!this.loaded) {
      this.cache = this.load();
      this.loaded = true;
    }
    return this.cache;
  }

  public set(state: RenewalState | undefined): void {
    this.cache = state;
    this.loaded = true;
    try {
      if (state === undefined) {
        fs.rmSync(this.filePath(), { force: true });
      } else {
        const sealed = safeStorage.encryptString(JSON.stringify(state));
        fs.writeFileSync(this.filePath(), sealed, { mode: 0o600 });
      }
    } catch (e) {
      log.error(`Failed to persist renewal state: ${(e as Error).message}`);
    }
  }

  private load(): RenewalState | undefined {
    try {
      if (!this.available() || !fs.existsSync(this.filePath())) {
        return undefined;
      }
      const sealed = fs.readFileSync(this.filePath());
      const parsed = JSON.parse(safeStorage.decryptString(sealed)) as RenewalState;
      if (
        typeof parsed.customerId !== 'string' ||
        typeof parsed.renewalToken !== 'string' ||
        typeof parsed.months !== 'number' ||
        typeof parsed.accountTag !== 'string'
      ) {
        return undefined;
      }
      return { ...parsed, attempt: parsed.attempt ?? 0 };
    } catch (e) {
      // A blob sealed by another OS user/keychain state decrypts to
      // garbage: treat as absent, the user can re-opt-in.
      log.warn(`Failed to load renewal state: ${(e as Error).message}`);
      return undefined;
    }
  }

  private filePath(): string {
    return path.join(app.getPath('userData'), FILE_NAME);
  }
}
