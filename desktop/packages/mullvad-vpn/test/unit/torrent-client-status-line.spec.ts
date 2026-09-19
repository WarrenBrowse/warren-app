import { describe, expect, it } from 'vitest';

import {
  torrentClientConfigErrorLine,
  torrentClientProbeLine,
  torrentClientStatusLine,
} from '../../src/renderer/features/port-forwarding/torrent-client';
import { TorrentClientPublicConfig } from '../../src/shared/torrent-client';

const config: TorrentClientPublicConfig = {
  kind: 'qbittorrent',
  url: 'http://127.0.0.1:8080',
  username: 'alice',
  hasPassword: true,
};

const at = 1_700_000_000_000;

describe('the torrent client status line', () => {
  it('names the port the client now listens on', () => {
    expect(torrentClientStatusLine({ state: 'synced', port: 58291, at }, config)).to.equal(
      'qBittorrent now listens on port 58291',
    );
  });

  it('says it is waiting while the exit holds no port', () => {
    expect(torrentClientStatusLine({ state: 'waiting', at }, config)).to.equal(
      'Waiting for a public port',
    );
  });

  it('shows nothing while the feature is off or a write is in the air', () => {
    expect(torrentClientStatusLine({ state: 'off', at }, config)).to.equal(undefined);
    expect(torrentClientStatusLine({ state: 'pushing', port: 58291, at }, config)).to.equal(
      undefined,
    );
  });

  it('names the address when the client cannot be reached', () => {
    expect(
      torrentClientStatusLine({ state: 'error', error: { kind: 'unreachable' }, at }, config),
    ).to.equal('Cannot reach qBittorrent at http://127.0.0.1:8080');
  });

  // An HTTP status the app cannot use means it did not find a working web API
  // at that address, which is the same thing to the person who has to fix it.
  it('reads an unusable answer as not reaching the client', () => {
    expect(
      torrentClientStatusLine(
        { state: 'error', error: { kind: 'bad-response', status: 502 }, at },
        config,
      ),
    ).to.equal('Cannot reach qBittorrent at http://127.0.0.1:8080');
  });

  it('says so when the client refused the login', () => {
    expect(
      torrentClientStatusLine({ state: 'error', error: { kind: 'login-refused' }, at }, config),
    ).to.equal('qBittorrent refused the login');
  });

  it("quotes the client's own refusal of the port", () => {
    expect(
      torrentClientStatusLine(
        { state: 'error', error: { kind: 'rejected', detail: 'Invalid listen_port' }, at },
        config,
      ),
    ).to.equal('qBittorrent rejected the port: Invalid listen_port');
  });
});

describe('the torrent client probe line', () => {
  it('names the client, its version and the port it listens on', () => {
    expect(torrentClientProbeLine({ version: 'v5.0.3', listenPort: 6881 }, config)).to.equal(
      'Connected to qBittorrent v5.0.3, listening on port 6881',
    );
  });

  it('drops the version when the client does not report one', () => {
    expect(torrentClientProbeLine({ version: undefined, listenPort: 6881 }, config)).to.equal(
      'qBittorrent now listens on port 6881',
    );
  });

  it('confirms the connection when the client reports no port', () => {
    expect(
      torrentClientProbeLine(
        { version: '2.1.1', listenPort: undefined },
        {
          ...config,
          kind: 'deluge',
        },
      ),
    ).to.equal('Connected to Deluge');
  });
});

describe('the torrent client configuration refusal', () => {
  it('says the password cannot be kept safely', () => {
    expect(torrentClientConfigErrorLine()).to.equal(
      'This system cannot store the password securely',
    );
  });
});
