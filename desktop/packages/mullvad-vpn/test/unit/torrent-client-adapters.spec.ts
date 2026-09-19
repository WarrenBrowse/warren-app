import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  createTorrentClientAdapter,
  TORRENT_CLIENT_TIMEOUT_MS,
} from '../../src/main/torrent-client/adapters';
import { TorrentClientError, TorrentClientKind } from '../../src/shared/torrent-client';

interface RecordedRequest {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: string;
}

type Responder = (request: RecordedRequest) => Response;

function headersToObject(headers: HeadersInit | undefined): Record<string, string> {
  const result: Record<string, string> = {};
  new Headers(headers).forEach((value, key) => {
    result[key.toLowerCase()] = value;
  });
  return result;
}

/** A `fetch` that records every request and answers the scripted responses in
 * order. An unscripted request is a test failure rather than a default
 * answer, so a leg the adapter should not fire cannot pass unnoticed. */
function scriptedFetch(responders: Responder[]) {
  const requests: RecordedRequest[] = [];
  const signals: (AbortSignal | undefined)[] = [];

  const fetchImpl = ((input: RequestInfo | URL, init?: RequestInit) => {
    const record: RecordedRequest = {
      method: init?.method ?? 'GET',
      url: String(input),
      headers: headersToObject(init?.headers),
      body: typeof init?.body === 'string' ? init.body : '',
    };
    requests.push(record);
    signals.push((init?.signal ?? undefined) as AbortSignal | undefined);
    const responder = responders[requests.length - 1];
    if (responder === undefined) {
      return Promise.reject(new Error(`unscripted request: ${record.method} ${record.url}`));
    }
    return Promise.resolve(responder(record));
  }) as unknown as typeof fetch;

  return { fetchImpl, requests, signals };
}

/** A fetch that fails the way an aborted request does. */
const timingOut = (() =>
  Promise.reject(
    new DOMException('The operation was aborted', 'TimeoutError'),
  )) as never as typeof fetch;

function textResponse(body: string, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(body, { status, headers });
}

function jsonResponse(body: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json', ...headers },
  });
}

async function failureOf(action: () => Promise<unknown>): Promise<TorrentClientError> {
  try {
    await action();
  } catch (error) {
    return (error as { failure: TorrentClientError }).failure;
  }
  throw new Error('expected the adapter to fail');
}

function adapter(
  kind: TorrentClientKind,
  fetchImpl: typeof fetch,
  overrides: { url?: string; username?: string; password?: string } = {},
) {
  return createTorrentClientAdapter({
    kind,
    url: overrides.url ?? 'http://127.0.0.1:8080',
    username: overrides.username ?? 'alice',
    password: overrides.password ?? 's3cr3t',
    fetch: fetchImpl,
  });
}

afterEach(() => vi.restoreAllMocks());

describe('the qBittorrent adapter', () => {
  it('logs in with a form body and carries the session cookie onwards', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () => textResponse('Ok.', 200, { 'set-cookie': 'SID=abc123; path=/; HttpOnly' }),
      () => textResponse('v5.0.3'),
      () => jsonResponse({ listen_port: 6881, random_port: false }),
    ]);

    const result = await adapter('qbittorrent', fetchImpl).probe();

    expect(requests[0].method).to.equal('POST');
    expect(requests[0].url).to.equal('http://127.0.0.1:8080/api/v2/auth/login');
    expect(requests[0].headers['content-type']).to.equal('application/x-www-form-urlencoded');
    expect(requests[0].body).to.equal('username=alice&password=s3cr3t');
    expect(requests[1].url).to.equal('http://127.0.0.1:8080/api/v2/app/version');
    expect(requests[1].headers['cookie']).to.equal('SID=abc123');
    expect(requests[2].url).to.equal('http://127.0.0.1:8080/api/v2/app/preferences');
    expect(result).to.deep.equal({ version: 'v5.0.3', listenPort: 6881 });
  });

  it('writes the listen port with the exact preferences body', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () => textResponse('Ok.'),
      () => textResponse(''),
    ]);

    await adapter('qbittorrent', fetchImpl).setListenPort(58291);

    expect(requests[1].url).to.equal('http://127.0.0.1:8080/api/v2/app/setPreferences');
    expect(requests[1].body).to.equal(
      'json=%7B%22listen_port%22%3A58291%2C%22random_port%22%3Afalse%2C%22upnp%22%3Afalse%7D',
    );
    expect(new URLSearchParams(requests[1].body).get('json')).to.equal(
      '{"listen_port":58291,"random_port":false,"upnp":false}',
    );
  });

  it('reports a refused login when the password is wrong', async () => {
    const { fetchImpl } = scriptedFetch([
      () => textResponse('Fails.'),
      () => textResponse('Forbidden', 403),
    ]);

    const failure = await failureOf(() => adapter('qbittorrent', fetchImpl).probe());

    expect(failure).to.deep.equal({ kind: 'login-refused' });
  });

  it('treats a refused login followed by a working API as logged in', async () => {
    const { fetchImpl } = scriptedFetch([
      () => textResponse('Forbidden', 403),
      () => textResponse('v4.6.7'),
      () => jsonResponse({ listen_port: 49200 }),
    ]);

    const result = await adapter('qbittorrent', fetchImpl).probe();

    expect(result).to.deep.equal({ version: 'v4.6.7', listenPort: 49200 });
  });

  it('reports a rejected port when qBittorrent refuses the preferences', async () => {
    const { fetchImpl } = scriptedFetch([
      () => textResponse('Ok.'),
      () => textResponse('Invalid listen_port', 400),
    ]);

    const failure = await failureOf(() => adapter('qbittorrent', fetchImpl).setListenPort(58291));

    expect(failure).to.deep.equal({ kind: 'rejected', detail: 'Invalid listen_port' });
  });

  it('reports a bad response when the client answers a status it cannot read', async () => {
    const { fetchImpl } = scriptedFetch([() => textResponse('Ok.'), () => textResponse('', 502)]);

    const failure = await failureOf(() => adapter('qbittorrent', fetchImpl).setListenPort(58291));

    expect(failure).to.deep.equal({ kind: 'bad-response', status: 502 });
  });

  it('tolerates a trailing slash on the address', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () => textResponse('Ok.'),
      () => textResponse(''),
    ]);

    await adapter('qbittorrent', fetchImpl, { url: 'http://127.0.0.1:8080///' }).setListenPort(
      58291,
    );

    expect(requests[0].url).to.equal('http://127.0.0.1:8080/api/v2/auth/login');
  });

  it('refuses an address that is not an http url, before any request', async () => {
    const { fetchImpl, requests } = scriptedFetch([]);

    const failure = await failureOf(() =>
      adapter('qbittorrent', fetchImpl, { url: 'ftp://127.0.0.1:8080' }).probe(),
    );

    expect(failure).to.deep.equal({ kind: 'unreachable' });
    expect(requests).to.have.length(0);
  });

  it('reports unreachable when the request times out', async () => {
    const timeout = vi.spyOn(AbortSignal, 'timeout');

    const failure = await failureOf(() => adapter('qbittorrent', timingOut).probe());

    expect(failure).to.deep.equal({ kind: 'unreachable' });
    expect(timeout).toHaveBeenCalledWith(TORRENT_CLIENT_TIMEOUT_MS);
    expect(TORRENT_CLIENT_TIMEOUT_MS).to.equal(10_000);
  });
});

describe('the Transmission adapter', () => {
  const rpcUrl = 'http://127.0.0.1:9091/transmission/rpc';

  it('retries the same request once with the session id the 409 hands back', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () => textResponse('', 409, { 'x-transmission-session-id': 'tok-42' }),
      () =>
        jsonResponse({
          result: 'success',
          arguments: { version: '4.0.5', 'peer-port': 51413 },
        }),
    ]);

    const result = await adapter('transmission', fetchImpl, {
      url: 'http://127.0.0.1:9091',
    }).probe();

    expect(requests).to.have.length(2);
    expect(requests[0].url).to.equal(rpcUrl);
    expect(requests[0].body).to.equal('{"method":"session-get"}');
    expect(requests[0].headers['x-transmission-session-id']).to.equal(undefined);
    expect(requests[1].body).to.equal(requests[0].body);
    expect(requests[1].headers['x-transmission-session-id']).to.equal('tok-42');
    expect(result).to.deep.equal({ version: '4.0.5', listenPort: 51413 });
  });

  it('keeps an address that already names the rpc endpoint', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () => jsonResponse({ result: 'success', arguments: {} }),
    ]);

    await adapter('transmission', fetchImpl, { url: `${rpcUrl}/` }).probe();

    expect(requests[0].url).to.equal(rpcUrl);
  });

  it('sends basic auth when a username is set, and none when it is empty', async () => {
    const withUser = scriptedFetch([() => jsonResponse({ result: 'success', arguments: {} })]);
    await adapter('transmission', withUser.fetchImpl, {
      url: 'http://127.0.0.1:9091',
      username: 'alice',
      password: 's3cr3t',
    }).probe();

    const withoutUser = scriptedFetch([() => jsonResponse({ result: 'success', arguments: {} })]);
    await adapter('transmission', withoutUser.fetchImpl, {
      url: 'http://127.0.0.1:9091',
      username: '',
      password: '',
    }).probe();

    expect(withUser.requests[0].headers['authorization']).to.equal(
      `Basic ${Buffer.from('alice:s3cr3t').toString('base64')}`,
    );
    expect(withoutUser.requests[0].headers['authorization']).to.equal(undefined);
  });

  it('writes the peer port with the exact session-set body', async () => {
    const { fetchImpl, requests } = scriptedFetch([() => jsonResponse({ result: 'success' })]);

    await adapter('transmission', fetchImpl, { url: 'http://127.0.0.1:9091' }).setListenPort(58291);

    expect(requests[0].body).to.equal(
      '{"method":"session-set","arguments":{"peer-port":58291,"peer-port-random-on-start":false,"port-forwarding-enabled":false}}',
    );
  });

  it('reports a refused login on 401', async () => {
    const { fetchImpl } = scriptedFetch([() => textResponse('Unauthorized', 401)]);

    const failure = await failureOf(() =>
      adapter('transmission', fetchImpl, { url: 'http://127.0.0.1:9091' }).probe(),
    );

    expect(failure).to.deep.equal({ kind: 'login-refused' });
  });

  it('reports a rejected port when a 200 answers anything but success', async () => {
    const { fetchImpl } = scriptedFetch([
      () => jsonResponse({ result: 'invalid argument: peer-port' }),
    ]);

    const failure = await failureOf(() =>
      adapter('transmission', fetchImpl, { url: 'http://127.0.0.1:9091' }).setListenPort(58291),
    );

    expect(failure).to.deep.equal({
      kind: 'rejected',
      detail: 'invalid argument: peer-port',
    });
  });

  it('reports unreachable when the request times out', async () => {
    const failure = await failureOf(() =>
      adapter('transmission', timingOut, { url: 'http://127.0.0.1:9091' }).probe(),
    );

    expect(failure).to.deep.equal({ kind: 'unreachable' });
  });
});

describe('the Deluge adapter', () => {
  const jsonUrl = 'http://127.0.0.1:8112/json';

  it('logs in with the password alone and carries the session cookie onwards', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () =>
        jsonResponse({ id: 1, result: true, error: null }, 200, {
          'set-cookie': '_session_id=xyz789; Path=/',
        }),
      () => jsonResponse({ id: 2, result: true, error: null }),
      () =>
        jsonResponse({
          id: 3,
          result: { listen_ports: [6881, 6881], random_port: false },
          error: null,
        }),
      () => jsonResponse({ id: 4, result: '2.1.1', error: null }),
    ]);

    const result = await adapter('deluge', fetchImpl, { url: 'http://127.0.0.1:8112' }).probe();

    expect(requests[0].url).to.equal(jsonUrl);
    expect(requests[0].headers['content-type']).to.equal('application/json');
    expect(requests[0].body).to.equal('{"method":"auth.login","params":["s3cr3t"],"id":1}');
    expect(requests[1].headers['cookie']).to.equal('_session_id=xyz789');
    expect(requests[1].body).to.equal('{"method":"web.connected","params":[],"id":2}');
    expect(requests[2].body).to.equal(
      '{"method":"core.get_config_values","params":[["listen_ports","random_port"]],"id":3}',
    );
    expect(requests[3].body).to.equal('{"method":"daemon.get_version","params":[],"id":4}');
    expect(result).to.deep.equal({ version: '2.1.1', listenPort: 6881 });
  });

  it('attaches the web interface to a daemon only when it is not attached yet', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () => jsonResponse({ id: 1, result: true, error: null }),
      () => jsonResponse({ id: 2, result: false, error: null }),
      () => jsonResponse({ id: 3, result: [['host-1', '127.0.0.1', 58846, 'localclient']] }),
      () => jsonResponse({ id: 4, result: true, error: null }),
      () => jsonResponse({ id: 5, result: { listen_ports: [6881, 6881] }, error: null }),
      () => jsonResponse({ id: 6, result: '2.1.1', error: null }),
    ]);

    await adapter('deluge', fetchImpl, { url: 'http://127.0.0.1:8112' }).probe();

    expect(requests[2].body).to.equal('{"method":"web.get_hosts","params":[],"id":3}');
    expect(requests[3].body).to.equal('{"method":"web.connect","params":["host-1"],"id":4}');
  });

  it('writes the listen ports ahead of clearing random_port', async () => {
    const { fetchImpl, requests } = scriptedFetch([
      () => jsonResponse({ id: 1, result: true, error: null }),
      () => jsonResponse({ id: 2, result: true, error: null }),
      () => jsonResponse({ id: 3, result: null, error: null }),
    ]);

    await adapter('deluge', fetchImpl, { url: 'http://127.0.0.1:8112' }).setListenPort(58291);

    expect(requests[2].body).to.equal(
      '{"method":"core.set_config","params":[{"listen_ports":[58291,58291],"random_port":false,"upnp":false,"natpmp":false}],"id":3}',
    );
  });

  it('reports a refused login when the password is wrong', async () => {
    const { fetchImpl } = scriptedFetch([
      () => jsonResponse({ id: 1, result: false, error: null }),
    ]);

    const failure = await failureOf(() =>
      adapter('deluge', fetchImpl, { url: 'http://127.0.0.1:8112' }).probe(),
    );

    expect(failure).to.deep.equal({ kind: 'login-refused' });
  });

  it('reports a rejected port when the answer carries an error', async () => {
    const { fetchImpl } = scriptedFetch([
      () => jsonResponse({ id: 1, result: true, error: null }),
      () => jsonResponse({ id: 2, result: true, error: null }),
      () =>
        jsonResponse({
          id: 3,
          result: null,
          error: { message: 'listen_ports must be a pair', code: 4 },
        }),
    ]);

    const failure = await failureOf(() =>
      adapter('deluge', fetchImpl, { url: 'http://127.0.0.1:8112' }).setListenPort(58291),
    );

    expect(failure).to.deep.equal({
      kind: 'rejected',
      detail: 'listen_ports must be a pair',
    });
  });

  it('reports unreachable when the request times out', async () => {
    const failure = await failureOf(() =>
      adapter('deluge', timingOut, { url: 'http://127.0.0.1:8112' }).probe(),
    );

    expect(failure).to.deep.equal({ kind: 'unreachable' });
  });
});

describe('every torrent client adapter', () => {
  // The detail of a `rejected` error is the client's own words, shown to the
  // user as is. A client that echoes what it was sent would otherwise put the
  // web interface password on screen, and into whatever the user pastes into
  // a bug report.
  it('keeps the credentials out of an error detail', async () => {
    const echo = () => textResponse('rejected for alice: the password hunter2 is not valid', 400);
    const { fetchImpl } = scriptedFetch([() => textResponse('Ok.'), echo]);

    const failure = await failureOf(() =>
      adapter('qbittorrent', fetchImpl, {
        username: 'alice',
        password: 'hunter2',
      }).setListenPort(58291),
    );

    expect(failure.kind).to.equal('rejected');
    const detail = (failure as { detail: string }).detail;
    expect(detail).to.not.contain('hunter2');
    expect(detail).to.not.contain('alice');
  });

  it('never puts a credential in the message of the error it throws', async () => {
    const { fetchImpl } = scriptedFetch([
      () => textResponse('Fails.'),
      () => textResponse('password hunter2 rejected for alice', 403),
    ]);

    let message = '';
    try {
      await adapter('qbittorrent', fetchImpl, { username: 'alice', password: 'hunter2' }).probe();
    } catch (error) {
      message = (error as Error).message;
    }

    expect(message).to.not.contain('hunter2');
    expect(message).to.not.contain('alice');
  });
});
