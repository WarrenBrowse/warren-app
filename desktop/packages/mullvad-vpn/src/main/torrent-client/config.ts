import { safeStorage } from 'electron';

import {
  StoredTorrentClient,
  TorrentClientConfig,
  TorrentClientConfigResult,
  TorrentClientConfigUpdate,
  TorrentClientPublicConfig,
  TorrentClientRuleRef,
} from '../../shared/torrent-client';

/**
 * Where the torrent client password is sealed.
 *
 * Injected so the store is testable without a keychain, and so the one place
 * that touches `safeStorage` stays a handful of lines.
 */
export interface SecretStore {
  readonly available: boolean;
  encrypt(plain: string): string;
  decrypt(cipher: string): string;
}

/** The platform keychain, through Electron's `safeStorage`. Base64 because
 * the sealed bytes go into a JSON file. */
export class SafeStorageSecretStore implements SecretStore {
  public get available(): boolean {
    try {
      if (!safeStorage.isEncryptionAvailable()) {
        return false;
      }
      // On Linux without a live keyring, safeStorage reports encryption as
      // available while sealing with a hardcoded key: cleartext-equivalent,
      // so refuse it rather than claim a password is protected when it is
      // not. Same gate as the forum identity store.
      return (
        process.platform !== 'linux' || safeStorage.getSelectedStorageBackend() !== 'basic_text'
      );
    } catch {
      return false;
    }
  }

  public encrypt(plain: string): string {
    return safeStorage.encryptString(plain).toString('base64');
  }

  public decrypt(cipher: string): string {
    return safeStorage.decryptString(Buffer.from(cipher, 'base64'));
  }
}

/**
 * The address as it is stored: scheme, host, port and path, nothing else.
 *
 * Userinfo (`http://user:pass@host`) is dropped rather than kept. The
 * adapters already ignore it, because they rebuild the base from the origin,
 * so keeping it would only put a password in a file that is cleartext, in the
 * renderer's store, and on screen in the line that names the address.
 * Anything that does not parse is stored trimmed and refused later, by the
 * adapter, which is the one place that decides what is reachable.
 */
function storedUrl(raw: string): string {
  const trimmed = raw.trim();
  try {
    const parsed = new URL(trimmed);
    return `${parsed.origin}${parsed.pathname}`.replace(/\/+$/, '');
  } catch {
    return trimmed;
  }
}

/** The host an address points at, or the address itself when it does not
 * parse. Compared across an update to decide whether a stored password is
 * still being handed to the machine it was typed for. */
function hostOf(raw: string): string {
  try {
    return new URL(raw).hostname;
  } catch {
    return raw.trim();
  }
}

const EMPTY_CONFIG: TorrentClientConfig = {
  kind: 'none',
  url: '',
  username: '',
  rule: undefined,
};

/**
 * The torrent client settings, with the password kept apart from everything
 * else that describes them.
 *
 * Three views come out of one stored blob, and only one of them ever carries
 * the password: `config()` for anything that needs to know which client is
 * configured, `publicConfig()` for the renderer, and `password()` for the one
 * call that is about to log in.
 */
export class TorrentClientConfigStore {
  public constructor(
    private readonly secrets: SecretStore,
    private readonly read: () => StoredTorrentClient | undefined,
    private readonly write: (value: StoredTorrentClient) => void,
  ) {}

  public config(): TorrentClientConfig {
    const stored = this.read();
    if (stored === undefined) {
      return EMPTY_CONFIG;
    }
    return {
      kind: stored.kind,
      url: stored.url,
      username: stored.username,
      rule: stored.rule,
    };
  }

  public publicConfig(): TorrentClientPublicConfig {
    const stored = this.read();
    return {
      ...this.config(),
      hasPassword: (stored?.passwordEncrypted ?? '') !== '',
    };
  }

  /** The password in the clear, for the call that is about to use it.
   * `undefined` when none is stored, and also when the sealed blob will not
   * open: a keychain entry sealed under another login cannot be recovered, and
   * a failed login is a better outcome than a crash on the push path. */
  public password(): string | undefined {
    const stored = this.read();
    if (stored === undefined || stored.passwordEncrypted === '') {
      return undefined;
    }
    try {
      return this.secrets.decrypt(stored.passwordEncrypted);
    } catch {
      return undefined;
    }
  }

  /**
   * Applies a change from the settings form.
   *
   * Only the leg that would write a secret is refused when the platform has
   * no real encryption: turning the feature off, or correcting an address,
   * stays possible on a machine with no keyring.
   */
  public update(update: TorrentClientConfigUpdate): TorrentClientConfigResult {
    const current = this.read();
    const url = storedUrl(update.url);
    let passwordEncrypted = current?.passwordEncrypted ?? '';

    // A password is sealed for the machine it was typed for. Pointing the
    // app at another host without typing a new one would otherwise hand that
    // password to whoever answers there, and an update is the only thing that
    // chooses where it goes.
    if (current !== undefined && hostOf(current.url) !== hostOf(url)) {
      passwordEncrypted = '';
    }

    if (update.password !== undefined) {
      if (update.password === '') {
        passwordEncrypted = '';
      } else {
        if (!this.secrets.available) {
          return { error: 'encryption-unavailable' };
        }
        try {
          passwordEncrypted = this.secrets.encrypt(update.password);
        } catch {
          // A keychain that reports itself available can still refuse to
          // seal (a locked login keyring). Refusing the whole update is the
          // only answer that does not claim a password was kept.
          return { error: 'encryption-unavailable' };
        }
      }
    }

    const stored: StoredTorrentClient = {
      kind: update.kind,
      url,
      username: update.username,
      passwordEncrypted,
      rule: update.rule,
    };
    this.write(stored);
    return { ...this.config(), hasPassword: passwordEncrypted !== '' };
  }

  /** Follows the linked rule when the controller re-points it, so the link
   * outlives a restart. Does nothing when no client is configured: there is
   * no blob to attach the rule to. */
  public setRule(rule: TorrentClientRuleRef) {
    const stored = this.read();
    if (stored === undefined) {
      return;
    }
    this.write({ ...stored, rule });
  }
}
