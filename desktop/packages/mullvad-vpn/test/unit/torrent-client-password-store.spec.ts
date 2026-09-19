import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest';

// The store reaches `electron` only through `SafeStorageSecretStore`, which
// these cases replace with a fake; the alias keeps the import graph loadable.
vi.mock('electron', () => ({ app: { getPath: () => '/nonexistent' } }));

import {
  SealedTorrentClientPasswordStore,
  SecretStore,
  TORRENT_CLIENT_PASSWORD_FILE,
} from '../../src/main/torrent-client/password-store';

const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-torrent-password-'));
const FILE = path.join(USER_DATA, TORRENT_CLIENT_PASSWORD_FILE);

afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));

beforeEach(() => fs.rmSync(FILE, { force: true }));

/** Stands in for the platform keychain. Seals by reversing the bytes, which
 * is enough to tell a sealed file from a cleartext one. */
function fakeSecrets(
  available = true,
): SecretStore & { opened: number; sealed: number; failOpen: boolean } {
  const secrets = {
    available,
    opened: 0,
    sealed: 0,
    failOpen: false,
    encrypt(plain: string): Buffer {
      secrets.sealed += 1;
      return Buffer.from([...Buffer.from(plain, 'utf8')].reverse());
    },
    decrypt(bytes: Buffer): string {
      secrets.opened += 1;
      if (secrets.failOpen) {
        throw new Error('the blob was sealed under another keychain state');
      }
      return Buffer.from([...bytes].reverse()).toString('utf8');
    },
  };
  return secrets;
}

function storeWith(secrets: SecretStore) {
  return new SealedTorrentClientPasswordStore(secrets, () => FILE);
}

describe('the sealed torrent client password', () => {
  it('lives in its own blob, next to the other sealed blobs', () => {
    expect(TORRENT_CLIENT_PASSWORD_FILE).to.equal('torrent-client.bin');
  });

  it('comes back in the clear, and never sits in the file in the clear', () => {
    const store = storeWith(fakeSecrets());

    store.set('hunter2');

    expect(store.get()).to.equal('hunter2');
    expect(fs.readFileSync(FILE).toString('utf8')).to.not.contain('hunter2');
    // Read back by a second store, so the answer comes off the disk rather
    // than out of the first one's memory.
    expect(storeWith(fakeSecrets()).get()).to.equal('hunter2');
  });

  it('reads no password when the file is not there', () => {
    expect(storeWith(fakeSecrets()).get()).to.equal(undefined);
    expect(fs.existsSync(FILE)).to.equal(false);
  });

  it('removes the file when the password is cleared', () => {
    const store = storeWith(fakeSecrets());
    store.set('hunter2');

    store.set(undefined);

    expect(store.get()).to.equal(undefined);
    expect(fs.existsSync(FILE)).to.equal(false);
  });

  // Fail closed, the way the renewal mandate does: a platform with no real
  // encryption gets no password at all rather than a cleartext one.
  it('refuses to store a password the system cannot seal', () => {
    const store = storeWith(fakeSecrets(false));

    expect(() => store.set('hunter2')).to.throw();
    expect(fs.existsSync(FILE)).to.equal(false);
  });

  it('still forgets a password on a system that cannot seal', () => {
    storeWith(fakeSecrets()).set('hunter2');

    storeWith(fakeSecrets(false)).set(undefined);

    expect(fs.existsSync(FILE)).to.equal(false);
  });

  // A blob sealed under another login decrypts to nothing usable. A failed
  // login is a better outcome than a crash on the push path.
  it('reads no password when the blob will not open', () => {
    storeWith(fakeSecrets()).set('hunter2');
    const broken = fakeSecrets();
    broken.failOpen = true;

    expect(storeWith(broken).get()).to.equal(undefined);
  });

  // Opening the blob is a keychain access, and on some platforms a prompt.
  it('opens the blob once and remembers it', () => {
    storeWith(fakeSecrets()).set('hunter2');
    const secrets = fakeSecrets();
    const store = storeWith(secrets);

    store.get();
    store.get();
    store.get();

    expect(secrets.opened).to.equal(1);
  });

  it('writes the file so only its owner can read it', () => {
    storeWith(fakeSecrets()).set('hunter2');

    expect(fs.statSync(FILE).mode & 0o777).to.equal(0o600);
  });
});
