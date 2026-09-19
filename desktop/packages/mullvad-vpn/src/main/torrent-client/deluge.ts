import { ProbeResult } from '../../shared/torrent-client';
import {
  readJsonBody,
  redactCredentials,
  TorrentClientAdapter,
  TorrentClientAdapterOptions,
  torrentClientBaseUrl,
  TorrentClientFailure,
  TorrentClientHttp,
} from './http';

/** Cap on a Deluge error message carried into the UI, same reason as the
 * cap on an HTTP body: a traceback is not a sentence. */
const MAX_DETAIL_CHARS = 200;

interface DelugeAnswer extends Record<string, unknown> {
  result?: unknown;
  error?: unknown;
}

/**
 * Deluge, over the web interface's JSON-RPC endpoint.
 *
 * Two things make Deluge different from the other two. It authenticates on a
 * password alone, and the web interface is a client of the daemon rather than
 * the daemon itself: a fresh `deluge-web`, which is what a container image
 * gives you, is attached to no daemon at all until someone attaches it, and
 * every `core.*` call fails until then.
 */
export class DelugeAdapter implements TorrentClientAdapter {
  private readonly http: TorrentClientHttp;
  private readonly secrets: string[];
  private nextId = 1;

  public constructor(private readonly options: TorrentClientAdapterOptions) {
    this.http = new TorrentClientHttp(torrentClientBaseUrl(options.url), options.fetch);
    this.secrets = [options.password, options.username];
  }

  public async probe(): Promise<ProbeResult> {
    await this.attach();
    // `core.get_config_values(keys)` is the exported method in
    // deluge/core/core.py; it answers a dict of the keys asked for.
    const values = await this.call('core.get_config_values', [['listen_ports', 'random_port']]);
    this.requireNoError(values);
    const config = (values.result ?? {}) as Record<string, unknown>;
    const ports = config['listen_ports'];
    return {
      version: await this.version(),
      listenPort: Array.isArray(ports) && typeof ports[0] === 'number' ? ports[0] : undefined,
    };
  }

  public async setListenPort(port: number): Promise<void> {
    await this.attach();
    // `core.set_config` (deluge/core/core.py) assigns the keys in the order
    // the dict carries them, and the handler each assignment triggers,
    // `__set_listen_on` in deluge/core/preferencesmanager.py, ignores
    // `listen_ports` while `random_port` is still true. So the ports go in
    // first and the flag that makes them take effect goes in last. The two
    // forwarding flags go with them: the exit holds the forward, and a client
    // asking the local router for another one only moves itself off the port.
    const answer = await this.call('core.set_config', [
      { listen_ports: [port, port], random_port: false, upnp: false, natpmp: false },
    ]);
    this.requireNoError(answer);
  }

  /** Signs in, and makes sure the web interface is talking to a daemon. */
  private async attach(): Promise<void> {
    const login = await this.call('auth.login', [this.options.password]);
    if (login.result !== true) {
      throw new TorrentClientFailure({ kind: 'login-refused' });
    }

    const connected = await this.call('web.connected', []);
    if (connected.result === true) {
      return;
    }

    const hosts = await this.call('web.get_hosts', []);
    this.requireNoError(hosts);
    const first =
      Array.isArray(hosts.result) && Array.isArray(hosts.result[0])
        ? hosts.result[0][0]
        : undefined;
    if (typeof first !== 'string') {
      throw new TorrentClientFailure({
        kind: 'rejected',
        detail: 'the web interface is not connected to a Deluge daemon',
      });
    }
    this.requireNoError(await this.call('web.connect', [first]));
  }

  /** The daemon version, for the line that confirms which client answered.
   * `daemon.get_version` is the exported method in deluge/core/daemon.py;
   * it is cosmetic, so a daemon that will not answer it costs nothing. */
  private async version(): Promise<string | undefined> {
    const answer = await this.call('daemon.get_version', []);
    return typeof answer.result === 'string' ? answer.result : undefined;
  }

  private async call(method: string, params: unknown[]): Promise<DelugeAnswer> {
    const response = await this.http.send({
      method: 'POST',
      path: '/json',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ method, params, id: this.nextId++ }),
    });
    if (response.status === 401 || response.status === 403) {
      throw new TorrentClientFailure({ kind: 'login-refused' });
    }
    if (!response.ok) {
      throw new TorrentClientFailure({ kind: 'bad-response', status: response.status });
    }
    return readJsonBody(response);
  }

  /** Deluge answers 200 whatever happened and puts the verdict in `error`,
   * which is null when the call went through. */
  private requireNoError(answer: DelugeAnswer) {
    const error = answer.error;
    if (error === null || error === undefined) {
      return;
    }
    const message =
      typeof error === 'object' && 'message' in error
        ? String((error as { message: unknown }).message)
        : String(error);
    throw new TorrentClientFailure({
      kind: 'rejected',
      detail: redactCredentials(message.slice(0, MAX_DETAIL_CHARS), this.secrets),
    });
  }
}
