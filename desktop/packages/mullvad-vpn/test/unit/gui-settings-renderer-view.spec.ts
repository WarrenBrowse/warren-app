import { describe, expect, it } from 'vitest';

import { NatPmpProto } from '../../src/shared/daemon-rpc-types';
import { guiSettingsForRenderer, IGuiSettingsState } from '../../src/shared/gui-settings-state';

const state: IGuiSettingsState = {
  preferredLocale: 'system',
  enableSystemNotifications: true,
  forumNotifications: true,
  portForwardingNotifications: true,
  torrentClient: {
    kind: 'qbittorrent',
    url: 'http://127.0.0.1:8080',
    username: 'alice',
    passwordEncrypted: 'c2VhbGVkLWJ5dGVz',
    rule: { internalPort: 58291, protocol: NatPmpProto.both },
  },
  autoConnect: false,
  monochromaticIcon: false,
  startMinimized: false,
  unpinnedWindow: false,
  browsedForSplitTunnelingApplications: ['/usr/bin/firefox'],
  changelogDisplayedForVersion: '1.2.0',
  updateDismissedForVersion: '',
  animateMap: true,
};

describe('the GUI settings the renderer is given', () => {
  // The whole settings object is pushed to the renderer on every change, and
  // this key is the only one in it that carries a credential. The renderer
  // works from `TorrentClientPublicConfig`, which says whether a password is
  // stored and never what it is.
  it('strips the sealed torrent client password', () => {
    const forRenderer = guiSettingsForRenderer(state) as Record<string, unknown>;

    expect(forRenderer.torrentClient).to.equal(undefined);
    expect(JSON.stringify(forRenderer)).to.not.contain('c2VhbGVkLWJ5dGVz');
  });

  it('keeps every other setting untouched', () => {
    const expected: Record<string, unknown> = { ...state };
    delete expected.torrentClient;

    expect(guiSettingsForRenderer(state)).to.deep.equal(expected);
  });

  it('leaves the state it was given alone', () => {
    guiSettingsForRenderer(state);

    expect(state.torrentClient?.passwordEncrypted).to.equal('c2VhbGVkLWJ5dGVz');
  });
});
