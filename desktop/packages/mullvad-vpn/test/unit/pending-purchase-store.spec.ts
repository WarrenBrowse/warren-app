import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest';

// The store reaches `electron` only through the secret store, which these
// cases replace with a fake; the alias keeps the import graph loadable.
vi.mock('electron', () => ({ app: { getPath: () => '/nonexistent' } }));

import { SealedPendingPurchaseStore } from '../../src/main/pending-purchase-store';
import { SecretStore } from '../../src/main/torrent-client/password-store';
import log from '../../src/shared/logging';
import { LogLevel } from '../../src/shared/logging-types';

const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-pending-purchases-'));
const FILE = path.join(USER_DATA, 'purchases.bin');

const logged: string[] = [];
log.addOutput({ level: LogLevel.debug, write: (_level, message) => void logged.push(message) });

afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));

beforeEach(() => {
  fs.rmSync(FILE, { force: true });
  logged.length = 0;
});

// A pending entry as the purchase flow writes it: the wpid, the pull secret
// that collects the voucher, the start time and the account tag. The secret
// is not a palindrome, so the fake seal (a byte reversal) cannot leave it
// readable.
const SECRET = '0123456789abcdef'.repeat(4);
const ENTRY = `${'a'.repeat(32)}${SECRET}:1750000000000:acct1`;

/** Stands in for the platform keychain. Seals by reversing the bytes, which
 * is enough to tell a sealed file from a cleartext one. */
function fakeSecrets(available = true): SecretStore & { opensTo?: string } {
  const secrets: SecretStore & { opensTo?: string } = {
    available,
    encrypt: (plain: string) => Buffer.from([...Buffer.from(plain, 'utf8')].reverse()),
    decrypt: (sealed: Buffer) =>
      secrets.opensTo ?? Buffer.from([...sealed].reverse()).toString('utf8'),
  };
  return secrets;
}

function storeWith(secrets: SecretStore) {
  return new SealedPendingPurchaseStore(secrets, () => FILE);
}

describe('the pending purchases', () => {
  it('survive a restart without the pull secret ever sitting in the file in the clear', () => {
    storeWith(fakeSecrets()).set([ENTRY]);

    expect(fs.readFileSync(FILE).toString('utf8')).not.toContain(SECRET);
    // Read back by a second store, so the answer comes off the disk rather
    // than out of the first one's memory.
    expect(storeWith(fakeSecrets()).get()).toEqual([ENTRY]);
  });

  it('leave no file behind once the last one is collected', () => {
    const store = storeWith(fakeSecrets());
    store.set([ENTRY]);

    store.set([]);

    expect(fs.existsSync(FILE)).toBe(false);
    expect(store.get()).toEqual([]);
  });

  // No real encryption (Linux without a keyring): the purchase is still
  // collected while the app runs, and nothing reaches the disk.
  it('stay in memory only on a system that cannot seal them', () => {
    const store = storeWith(fakeSecrets(false));

    store.set([ENTRY]);

    expect(fs.existsSync(FILE)).toBe(false);
    expect(store.get()).toEqual([ENTRY]);
  });

  // A keyring locked for this session will open again: the purchases it
  // sealed are not this run's to throw away.
  it('leave a blob the system cannot open now to the keychain that sealed it', () => {
    storeWith(fakeSecrets()).set([ENTRY]);
    const locked = storeWith(fakeSecrets(false));

    expect(locked.get()).toEqual([]);
    locked.set([]);

    expect(storeWith(fakeSecrets()).get()).toEqual([ENTRY]);
  });

  it('are read as none from a blob that does not hold a list of entries', () => {
    storeWith(fakeSecrets()).set([ENTRY]);
    const secrets = fakeSecrets();
    secrets.opensTo = JSON.stringify({ entries: [ENTRY] });

    expect(storeWith(secrets).get()).toEqual([]);
  });

  // A parse error quotes the text around the point where it choked, and in
  // this blob that text is the pull secret.
  it('never reach a log line when their blob will not parse', () => {
    storeWith(fakeSecrets()).set([ENTRY]);
    const secrets = fakeSecrets();
    secrets.opensTo = `[x${SECRET}]`;

    expect(storeWith(secrets).get()).toEqual([]);
    expect(logged).not.toHaveLength(0);
    expect(logged.join('\n')).not.toContain(SECRET.slice(0, 8));
  });

  it('are written so only their owner can read the file', () => {
    storeWith(fakeSecrets()).set([ENTRY]);

    expect(fs.statSync(FILE).mode & 0o777).toBe(0o600);
  });
});
