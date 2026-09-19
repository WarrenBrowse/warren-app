import { ProbeResult } from '../../shared/torrent-client';
import {
  credentialForms,
  readJsonBody,
  redactDetail,
  TorrentClientAdapter,
  TorrentClientAdapterOptions,
  torrentClientBaseUrl,
  TorrentClientFailure,
  TorrentClientHttp,
} from './http';

/**
 * Transmission, over its RPC endpoint.
 *
 * The port lives in the session, so one `session-set` carries the number and
 * the two flags that would otherwise take it back: Transmission's own port
 * randomisation at start, and its NAT-PMP/UPnP client, which would ask the
 * local router for a forward the exit already holds.
 *
 * The names are the 4.0.x kebab-case ones, which 4.1 still accepts.
 */
export class TransmissionAdapter implements TorrentClientAdapter {
  private readonly http: TorrentClientHttp;
  private readonly headers: Record<string, string>;
  private readonly secrets: string[];
  private sessionId?: string;

  public constructor(options: TorrentClientAdapterOptions) {
    this.http = new TorrentClientHttp(
      rpcEndpoint(torrentClientBaseUrl(options.url)),
      options.fetch,
    );
    this.headers = { 'content-type': 'application/json' };
    if (options.username !== '') {
      const credentials = Buffer.from(`${options.username}:${options.password}`).toString('base64');
      this.headers['authorization'] = `Basic ${credentials}`;
    }
    this.secrets = credentialForms(options.username, options.password);
  }

  public async probe(): Promise<ProbeResult> {
    const answer = await this.call({ method: 'session-get' });
    this.requireSuccess(answer);
    const session = (answer['arguments'] ?? {}) as Record<string, unknown>;
    const version = session['version'];
    const peerPort = session['peer-port'];
    return {
      version: typeof version === 'string' ? version : undefined,
      listenPort: typeof peerPort === 'number' ? peerPort : undefined,
    };
  }

  public async setListenPort(port: number): Promise<void> {
    const answer = await this.call({
      method: 'session-set',
      arguments: {
        'peer-port': port,
        'peer-port-random-on-start': false,
        'port-forwarding-enabled': false,
      },
    });
    this.requireSuccess(answer);
  }

  /**
   * One RPC call, including the CSRF handshake.
   *
   * Transmission answers the first request of a session with 409 and the
   * token it wants echoed back. The same request then goes through, so the
   * retry replays the body rather than starting the exchange again, and it
   * happens once: a second 409 is a server that is not playing the protocol.
   */
  private async call(payload: Record<string, unknown>): Promise<Record<string, unknown>> {
    const body = JSON.stringify(payload);
    let response = await this.post(body);
    if (response.status === 409) {
      const token = response.headers.get('x-transmission-session-id');
      if (token !== null) {
        this.sessionId = token;
        response = await this.post(body);
      }
    }
    if (response.status === 401 || response.status === 403) {
      throw new TorrentClientFailure({ kind: 'login-refused' });
    }
    if (!response.ok) {
      throw new TorrentClientFailure({ kind: 'bad-response', status: response.status });
    }
    return readJsonBody(response);
  }

  private post(body: string): Promise<Response> {
    const headers =
      this.sessionId === undefined
        ? this.headers
        : { ...this.headers, 'x-transmission-session-id': this.sessionId };
    return this.http.send({ method: 'POST', headers, body });
  }

  /** Transmission answers a refused argument with 200 and a `result` naming
   * it, so the HTTP status says nothing about whether the port was applied. */
  private requireSuccess(answer: Record<string, unknown>) {
    const result = answer['result'];
    if (result === 'success') {
      return;
    }
    throw new TorrentClientFailure({
      kind: 'rejected',
      detail: redactDetail(typeof result === 'string' ? result : '', this.secrets),
    });
  }
}

/** The RPC endpoint under a web interface address, unless the address already
 * names it (which is how a reverse proxy in front of Transmission is usually
 * written down). */
function rpcEndpoint(base: string): string {
  return base.endsWith('/rpc') ? base : `${base}/transmission/rpc`;
}
