import { ProbeResult } from '../../shared/torrent-client';
import {
  detailFrom,
  readJsonBody,
  TorrentClientAdapter,
  TorrentClientAdapterOptions,
  torrentClientBaseUrl,
  TorrentClientFailure,
  TorrentClientHttp,
} from './http';

/**
 * qBittorrent, over its Web API v2.
 *
 * The incoming port lives in the application preferences, which are read and
 * written as a whole through `app/preferences` and `app/setPreferences`. The
 * write also turns qBittorrent's own port randomisation and UPnP off: the
 * exit already holds the forward, and either of those would move the client
 * off the granted port within the hour.
 */
export class QBittorrentAdapter implements TorrentClientAdapter {
  private readonly http: TorrentClientHttp;
  private readonly secrets: string[];

  public constructor(private readonly options: TorrentClientAdapterOptions) {
    this.http = new TorrentClientHttp(torrentClientBaseUrl(options.url), options.fetch);
    this.secrets = [options.password, options.username];
  }

  public async probe(): Promise<ProbeResult> {
    await this.login();
    const version = (await (await this.get('/api/v2/app/version')).text()).trim();
    const preferences = await readJsonBody(await this.get('/api/v2/app/preferences'));
    const listenPort = preferences['listen_port'];
    return {
      version: version === '' ? undefined : version,
      listenPort: typeof listenPort === 'number' ? listenPort : undefined,
    };
  }

  public async setListenPort(port: number): Promise<void> {
    await this.login();
    const response = await this.http.send({
      method: 'POST',
      path: '/api/v2/app/setPreferences',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ json: preferencesBody(port) }).toString(),
    });
    await this.ensureUsable(response);
  }

  /**
   * Opens a session, and tolerates being refused one.
   *
   * With `bypass_local_auth` on, which is the default for a client reached
   * over the loopback, qBittorrent answers every API call while refusing to
   * mint a session at all. So a refused login proves nothing on its own: the
   * call that follows decides, and answers 403 when we really are locked out.
   */
  private async login(): Promise<void> {
    const response = await this.http.send({
      method: 'POST',
      path: '/api/v2/auth/login',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({
        username: this.options.username,
        password: this.options.password,
      }).toString(),
    });
    if (response.status === 401 || response.status === 403) {
      return;
    }
    if (!response.ok) {
      throw new TorrentClientFailure({ kind: 'bad-response', status: response.status });
    }
    // Drains the body of the 200 that carries `Ok.` or `Fails.`; the verdict
    // is the next call's, not this one's.
    await response.text();
  }

  private async get(path: string): Promise<Response> {
    const response = await this.http.send({ method: 'GET', path });
    await this.ensureUsable(response);
    return response;
  }

  private async ensureUsable(response: Response): Promise<void> {
    if (response.status === 401 || response.status === 403) {
      throw new TorrentClientFailure({ kind: 'login-refused' });
    }
    // qBittorrent answers a preference it will not apply with 400 and a
    // sentence naming the field, which is worth showing to the user as is.
    if (response.status === 400) {
      throw new TorrentClientFailure({
        kind: 'rejected',
        detail: await detailFrom(response, this.secrets),
      });
    }
    if (!response.ok) {
      throw new TorrentClientFailure({ kind: 'bad-response', status: response.status });
    }
  }
}

/**
 * The preferences document qBittorrent is asked to apply.
 *
 * `random_port` has to be false in the same write: `setPreferencesAction`
 * (src/webui/api/appcontroller.cpp) applies `listen_port` only in the `else`
 * branch of a truthy `random_port`, so a client left on a random port would
 * ignore the number entirely.
 */
function preferencesBody(port: number): string {
  return JSON.stringify({ listen_port: port, random_port: false, upnp: false });
}
