import { app, safeStorage } from 'electron';
import fs from 'fs';
import path from 'path';

import {
  ForumIdentity,
  parseForumIdentity,
  serializeForumIdentity,
} from '../shared/forum-identity';
import log from '../shared/logging';

// OS-keychain-encrypted file store for the community-forum identity: the
// public handle, and the slot this installation occupies in the broadcast
// activity digest.
//
// The handle is public on the forum, but the LINK between it and this
// installation's wallet is not: written in the clear it would hand any local
// reader the pairwise identifier that warren-connect deliberately stops
// storing server side. The slot belongs in the same blob for the same
// reason: on its own it names nobody, but beside the handle it is half of
// that link. Both therefore get the same treatment as the renewal mandate
// (their own sealed blob under userData, never gui_settings.json, which is
// cleartext by invariant) rather than a settings key.
//
// Unlike the renewal token this is not a credential: when the OS offers no
// real encryption nothing is persisted, so the account view shows the handle
// for the rest of the session and again after the next forum login.

const FILE_NAME = 'forum.bin';

export interface ForumIdentityStore {
  get(): ForumIdentity | undefined;
  set(identity: ForumIdentity | undefined): void;
}

export default class SafeStorageForumIdentityStore implements ForumIdentityStore {
  private cache?: ForumIdentity;
  private loaded = false;

  public get(): ForumIdentity | undefined {
    if (!this.loaded) {
      this.cache = this.load();
      this.loaded = true;
    }
    return this.cache;
  }

  public set(identity: ForumIdentity | undefined): void {
    this.cache = identity;
    this.loaded = true;
    try {
      if (identity === undefined) {
        fs.rmSync(this.filePath(), { force: true });
      } else if (this.available()) {
        fs.writeFileSync(
          this.filePath(),
          safeStorage.encryptString(serializeForumIdentity(identity)),
          {
            mode: 0o600,
          },
        );
      }
    } catch (e) {
      log.error(`Failed to persist forum identity: ${(e as Error).message}`);
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

  private load(): ForumIdentity | undefined {
    try {
      if (!this.available() || !fs.existsSync(this.filePath())) {
        return undefined;
      }
      // The codec applies the same shape gate as the login response: a blob
      // sealed under another keychain state decrypts to garbage, which must
      // never reach the UI.
      return parseForumIdentity(safeStorage.decryptString(fs.readFileSync(this.filePath())));
    } catch (e) {
      log.warn(`Failed to load forum identity: ${(e as Error).message}`);
      return undefined;
    }
  }

  private filePath(): string {
    return path.join(app.getPath('userData'), FILE_NAME);
  }
}
