import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, describe, expect, it, vi } from 'vitest';

// GuiSettings resolves its file through `app.getPath`, and the unit suite
// aliases `electron` to an empty module.
const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-torrent-client-'));
vi.mock('electron', () => ({ app: { getPath: () => USER_DATA } }));

import GuiSettings from '../../src/main/gui-settings';
import { TorrentClientConfigStore } from '../../src/main/torrent-client/config';
import { TorrentClientPasswordStore } from '../../src/main/torrent-client/password-store';
import { NatPmpProto } from '../../src/shared/daemon-rpc-types';
import {
  isTorrentClientConfigError,
  parseStoredTorrentClient,
  StoredTorrentClient,
} from '../../src/shared/torrent-client';

/** Stands in for the sealed blob under the user-data directory (exercised
 * for real in torrent-client-password-store.spec.ts). */
function fakePasswords(
  available = true,
): TorrentClientPasswordStore & { held(): string | undefined } {
  let held: string | undefined;
  return {
    held: () => held,
    get: () => held,
    set: (password: string | undefined) => {
      if (password !== undefined && !available) {
        throw new Error('the platform offers no real encryption');
      }
      held = password;
    },
  };
}

function storeWith(passwords: ReturnType<typeof fakePasswords>, initial?: StoredTorrentClient) {
  const holder: { value?: StoredTorrentClient } = { value: initial };
  const store = new TorrentClientConfigStore(
    passwords,
    () => holder.value,
    (value) => {
      holder.value = value;
    },
  );
  return { store, holder, passwords };
}

// At file scope: an `afterAll` inside a describe fires when that block ends,
// which would take the directory away from the blocks after it.
afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));

describe('the torrent client configuration store', () => {
  // The settings file is cleartext by invariant, the same invariant the
  // renewal mandate and the forum identity state: nothing that has to be
  // protected goes in it, sealed or not.
  it('keeps the password out of the settings file entirely', () => {
    const { store, holder, passwords } = storeWith(fakePasswords());

    const result = store.update({
      kind: 'qbittorrent',
      url: '  http://127.0.0.1:8080  ',
      username: 'alice',
      password: 'hunter2',
    });

    expect(isTorrentClientConfigError(result)).to.equal(false);
    expect(Object.keys(holder.value ?? {})).to.deep.equal(['kind', 'url', 'username', 'rule']);
    expect(JSON.stringify(holder.value)).to.not.contain('hunter2');
    expect(passwords.held()).to.equal('hunter2');
    expect(store.password()).to.equal('hunter2');
    expect(store.config()).to.deep.equal({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      rule: undefined,
    });
  });

  it('keeps the stored password when the update carries none', () => {
    const { store } = storeWith(fakePasswords());
    store.update({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      password: 'hunter2',
    });

    store.update({ kind: 'qbittorrent', url: 'http://127.0.0.1:9090', username: 'bob' });

    expect(store.password()).to.equal('hunter2');
    expect(store.publicConfig().hasPassword).to.equal(true);
    expect(store.publicConfig().url).to.equal('http://127.0.0.1:9090');
  });

  it('clears the stored password on an empty one', () => {
    const { store } = storeWith(fakePasswords());
    store.update({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      password: 'hunter2',
    });

    const result = store.update({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      password: '',
    });

    expect(store.password()).to.equal(undefined);
    expect(store.publicConfig().hasPassword).to.equal(false);
    // The answer the form redraws from has to agree with the store, or the
    // field keeps its "Saved" placeholder over nothing.
    expect((result as { hasPassword: boolean }).hasPassword).to.equal(false);
  });

  // The renderer draws the settings form from this object. A password in it
  // would be readable in the devtools of any window that ever opened the view.
  it('never carries the password into the view the renderer gets', () => {
    const { store } = storeWith(fakePasswords());

    const result = store.update({
      kind: 'transmission',
      url: 'http://127.0.0.1:9091',
      username: 'alice',
      password: 'hunter2',
    });

    expect(result).to.deep.equal({
      kind: 'transmission',
      url: 'http://127.0.0.1:9091',
      username: 'alice',
      hasPassword: true,
      rule: undefined,
    });
    expect(JSON.stringify(result)).to.not.contain('hunter2');
  });

  it('refuses to store a password the system cannot seal', () => {
    const { store, holder } = storeWith(fakePasswords(false));

    const result = store.update({
      kind: 'deluge',
      url: 'http://127.0.0.1:8112',
      username: '',
      password: 'hunter2',
    });

    expect(result).to.deep.equal({ error: 'encryption-unavailable' });
    expect(holder.value).to.equal(undefined);
  });

  // Turning the feature off, or correcting the address, must stay possible on
  // a machine with no keyring: only the leg that would write a secret is
  // refused.
  it('still accepts an update that stores no password when sealing is unavailable', () => {
    const { store, holder } = storeWith(fakePasswords(false));

    const result = store.update({ kind: 'none', url: '', username: '' });

    expect(isTorrentClientConfigError(result)).to.equal(false);
    expect(holder.value?.kind).to.equal('none');
  });

  it('re-points the linked rule without touching the password', () => {
    const { store } = storeWith(fakePasswords());
    store.update({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      password: 'hunter2',
      rule: { internalPort: 6881, protocol: NatPmpProto.both },
    });

    store.setRule({ internalPort: 58291, protocol: NatPmpProto.both });

    expect(store.config().rule).to.deep.equal({
      internalPort: 58291,
      protocol: NatPmpProto.both,
    });
    expect(store.password()).to.equal('hunter2');
  });

  // A password sealed for one host must not be handed to another: an update
  // is the only thing that chooses where the app sends it, and a renderer that
  // sends a new address with no new password would otherwise redirect the
  // stored one at a host of its choosing.
  it('clears the stored password when the address moves to another host', () => {
    const { store } = storeWith(fakePasswords());
    store.update({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      password: 'hunter2',
    });

    store.update({ kind: 'qbittorrent', url: 'http://attacker.example', username: 'alice' });

    expect(store.password()).to.equal(undefined);
    expect(store.publicConfig().hasPassword).to.equal(false);
  });

  it('keeps the stored password when only the port is corrected', () => {
    const { store } = storeWith(fakePasswords());
    store.update({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      password: 'hunter2',
    });

    store.update({ kind: 'qbittorrent', url: 'http://127.0.0.1:9090', username: 'alice' });

    expect(store.password()).to.equal('hunter2');
  });

  // `http://user:pass@host` would put a password in a file that is cleartext,
  // in the renderer's store, and on screen in the "cannot reach it" line.
  it('drops credentials embedded in the address', () => {
    const { store, holder } = storeWith(fakePasswords());

    store.update({
      kind: 'qbittorrent',
      url: 'http://alice:hunter2@127.0.0.1:8080/',
      username: 'alice',
    });

    expect(store.config().url).to.equal('http://127.0.0.1:8080');
    expect(JSON.stringify(holder.value)).to.not.contain('hunter2');
  });

  it('refuses the update when sealing throws', () => {
    const throwing = fakePasswords();
    throwing.set = () => {
      throw new Error('the keychain is locked');
    };
    const { store, holder } = storeWith(throwing);

    const result = store.update({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      password: 'hunter2',
    });

    expect(result).to.deep.equal({ error: 'encryption-unavailable' });
    expect(holder.value).to.equal(undefined);
  });

  it('reads nothing at all as no client configured', () => {
    const { store } = storeWith(fakePasswords());

    expect(store.config().kind).to.equal('none');
    expect(store.publicConfig().hasPassword).to.equal(false);
    expect(store.password()).to.equal(undefined);
  });
});

describe('the stored torrent client blob', () => {
  it('accepts the shape the app writes', () => {
    const stored = {
      kind: 'deluge',
      url: 'http://127.0.0.1:8112',
      username: '',
      rule: { internalPort: 6881, protocol: 'both' },
    };

    expect(parseStoredTorrentClient(stored)).to.deep.equal(stored);
  });

  // The build that kept the password in this file never reached a user, so
  // nothing is owed to it; a settings file it wrote must still load, with the
  // key it left behind going nowhere.
  it('ignores a password left in a settings file by an earlier build', () => {
    const parsed = parseStoredTorrentClient({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      passwordEncrypted: 'sealed:aHVudGVyMg==',
    });

    expect(parsed).to.deep.equal({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      rule: undefined,
    });
    expect(JSON.stringify(parsed)).to.not.contain('sealed');
  });

  it('drops a blob whose client is not one the app speaks', () => {
    expect(
      parseStoredTorrentClient({
        kind: 'rtorrent',
        url: 'http://127.0.0.1:8080',
        username: '',
      }),
    ).to.equal(undefined);
  });

  it('drops a malformed rule and keeps the rest', () => {
    const parsed = parseStoredTorrentClient({
      kind: 'qbittorrent',
      url: 'http://127.0.0.1:8080',
      username: 'alice',
      rule: { internalPort: 'six thousand', protocol: 'both' },
    });

    expect(parsed?.kind).to.equal('qbittorrent');
    expect(parsed?.rule).to.equal(undefined);
  });

  it('drops anything that is not an object', () => {
    expect(parseStoredTorrentClient(undefined)).to.equal(undefined);
    expect(parseStoredTorrentClient(null)).to.equal(undefined);
    expect(parseStoredTorrentClient('qbittorrent')).to.equal(undefined);
  });
});

describe('the torrent client key in the GUI settings file', () => {
  // The key is absent from every settings file written before this lot, and
  // an absent key must not fail validation and reset the whole file.
  it('reads a settings file written before the key existed', () => {
    fs.writeFileSync(
      path.join(USER_DATA, 'gui_settings.json'),
      JSON.stringify({ enableSystemNotifications: false }),
    );

    const settings = new GuiSettings();
    settings.load();

    expect(settings.torrentClient).to.equal(undefined);
    expect(settings.enableSystemNotifications).to.equal(false);
  });

  it('survives a reload', () => {
    const settings = new GuiSettings();
    settings.load();
    settings.torrentClient = {
      kind: 'transmission',
      url: 'http://127.0.0.1:9091',
      username: 'alice',
    };

    const reloaded = new GuiSettings();
    reloaded.load();

    expect(reloaded.torrentClient).to.deep.equal({
      kind: 'transmission',
      url: 'http://127.0.0.1:9091',
      username: 'alice',
      rule: undefined,
    });
  });
});
