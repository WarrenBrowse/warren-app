import { app, safeStorage } from 'electron';
import fs from 'fs';
import path from 'path';

import log from '../shared/logging';

// OS-keychain-encrypted file store for the community-forum handle.
//
// The handle is public on the forum, but the LINK between it and this
// installation's wallet is not: written in the clear it would hand any local
// reader the pairwise identifier that warren-connect deliberately stops
// storing server side. It therefore gets the same treatment as the renewal
// mandate (its own sealed blob under userData, never gui_settings.json, which
// is cleartext by invariant) rather than a settings key.
//
// Unlike the renewal token this is not a credential: when the OS offers no
// real encryption the handle is simply not persisted, so the account view
// shows it for the rest of the session and again after the next forum login.

const FILE_NAME = 'forum.bin';

export interface ForumHandleStore {
  get(): string | undefined;
  set(handle: string | undefined): void;
}

export default class SafeStorageForumHandleStore implements ForumHandleStore {
  private cache?: string;
  private loaded = false;

  public get(): string | undefined {
    if (!this.loaded) {
      this.cache = this.load();
      this.loaded = true;
    }
    return this.cache;
  }

  public set(handle: string | undefined): void {
    this.cache = handle;
    this.loaded = true;
    try {
      if (handle === undefined) {
        fs.rmSync(this.filePath(), { force: true });
      } else if (this.available()) {
        fs.writeFileSync(this.filePath(), safeStorage.encryptString(handle), { mode: 0o600 });
      }
    } catch (e) {
      log.error(`Failed to persist forum handle: ${(e as Error).message}`);
    }
  }

  private available(): boolean {
    try {
      if (!safeStorage.isEncryptionAvailable()) {
        return false;
      }
      // On Linux without a live keyring, safeStorage reports encryption as
      // available while sealing with a hardcoded key: cleartext-equivalent,
      // so refuse it rather than write the wallet-to-handle link in the open.
      return (
        process.platform !== 'linux' || safeStorage.getSelectedStorageBackend() !== 'basic_text'
      );
    } catch {
      return false;
    }
  }

  private load(): string | undefined {
    try {
      if (!this.available() || !fs.existsSync(this.filePath())) {
        return undefined;
      }
      const handle = safeStorage.decryptString(fs.readFileSync(this.filePath()));
      // Same shape gate as the login response: a blob sealed under another
      // keychain state decrypts to garbage, which must never reach the UI.
      return /^[a-z]{5}-[a-z]{5}-[a-z]{5}$/.test(handle) ? handle : undefined;
    } catch (e) {
      log.warn(`Failed to load forum handle: ${(e as Error).message}`);
      return undefined;
    }
  }

  private filePath(): string {
    return path.join(app.getPath('userData'), FILE_NAME);
  }
}
