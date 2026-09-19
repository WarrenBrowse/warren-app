import { safeStorage } from 'electron';
import fs from 'fs';

import log from '../../shared/logging';

/**
 * The sealed blob holding the torrent client web interface password, beside
 * `renewal.bin` and `forum.bin` under the app's user-data directory.
 *
 * It is a file of its own rather than a key in `gui_settings.json` because
 * that file is cleartext by invariant, and a sealed value living in it would
 * make the next reader wonder which of the two rules is the real one.
 */
export const TORRENT_CLIENT_PASSWORD_FILE = 'torrent-client.bin';

/**
 * The platform keychain.
 *
 * Injected so the store is testable without one, and so the single place that
 * touches `safeStorage` stays a handful of lines.
 */
export interface SecretStore {
  readonly available: boolean;
  encrypt(plain: string): Buffer;
  decrypt(sealed: Buffer): string;
}

/** Electron's `safeStorage`: Keychain on macOS, DPAPI on Windows,
 * kwallet or libsecret on Linux. */
export class SafeStorageSecretStore implements SecretStore {
  public get available(): boolean {
    try {
      if (!safeStorage.isEncryptionAvailable()) {
        return false;
      }
      // On Linux without a live keyring, safeStorage reports encryption as
      // available while sealing with a hardcoded key: cleartext-equivalent,
      // so refuse it rather than claim a password is protected when it is
      // not. Same gate as the renewal mandate and the forum identity.
      return (
        process.platform !== 'linux' || safeStorage.getSelectedStorageBackend() !== 'basic_text'
      );
    } catch {
      return false;
    }
  }

  public encrypt(plain: string): Buffer {
    return safeStorage.encryptString(plain);
  }

  public decrypt(sealed: Buffer): string {
    return safeStorage.decryptString(sealed);
  }
}

/**
 * Where the torrent client password is kept. The configuration store holds
 * one of these and never sees a sealed byte.
 *
 * There is no "can you seal this" question on this interface on purpose: the
 * answer would be a second place deciding what fails closed, and the two
 * would drift. `set` refuses, and the caller reports the refusal.
 */
export interface TorrentClientPasswordStore {
  get(): string | undefined;
  /** Stores a password, or forgets the stored one when given `undefined`.
   * Throws rather than write a password the platform cannot seal. */
  set(password: string | undefined): void;
}

/**
 * The password, sealed by the platform keychain, in a file of its own.
 *
 * Fails closed: a platform with no real encryption gets no stored password at
 * all, which costs the user a re-typed password and never puts one on disk in
 * the clear. The path is injected so the store can be exercised against a
 * temporary directory.
 */
export class SealedTorrentClientPasswordStore implements TorrentClientPasswordStore {
  private cache?: string;
  private loaded = false;

  public constructor(
    private readonly secrets: SecretStore,
    private readonly filePath: () => string,
  ) {}

  public get available(): boolean {
    return this.secrets.available;
  }

  /** The password in the clear. Opening the blob is a keychain access, and on
   * some platforms a prompt, so it happens once per run: this process is the
   * only writer. */
  public get(): string | undefined {
    if (!this.loaded) {
      this.cache = this.load();
      this.loaded = true;
    }
    return this.cache;
  }

  public set(password: string | undefined) {
    if (password === undefined) {
      this.remember(undefined);
      try {
        fs.rmSync(this.filePath(), { force: true });
      } catch (e) {
        log.error(`Failed to remove the torrent client password: ${(e as Error).message}`);
      }
      return;
    }

    if (!this.available) {
      throw new Error('the platform offers no real encryption');
    }
    fs.writeFileSync(this.filePath(), this.secrets.encrypt(password), { mode: 0o600 });
    this.remember(password);
  }

  private remember(password: string | undefined) {
    this.cache = password;
    this.loaded = true;
  }

  private load(): string | undefined {
    try {
      if (!this.available || !fs.existsSync(this.filePath())) {
        return undefined;
      }
      return this.secrets.decrypt(fs.readFileSync(this.filePath()));
    } catch (e) {
      // A blob sealed under another keychain state cannot be recovered. The
      // message is the platform's, never the value.
      log.warn(`Failed to read the torrent client password: ${(e as Error).message}`);
      return undefined;
    }
  }
}
