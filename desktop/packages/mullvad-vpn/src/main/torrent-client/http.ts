import { ProbeResult, TorrentClientError, TorrentClientKind } from '../../shared/torrent-client';

/**
 * Every request the app makes to a torrent client gives up after this long.
 *
 * A torrent client that stopped answering is indistinguishable from one that
 * is not there, and the port push runs unattended behind a NAT-PMP event: a
 * request left hanging would hold the retry chain open for as long as the
 * client stays wedged.
 */
export const TORRENT_CLIENT_TIMEOUT_MS = 10_000;

/** Cap on the client's own words carried into a `rejected` error. Long enough
 * for a sentence, short enough that a client answering an HTML page cannot
 * paste a document into the settings view. */
const MAX_DETAIL_CHARS = 200;

/** What every adapter throws. The sealed {@link TorrentClientError} rides on
 * the exception rather than the message, so the renderer phrases the case in
 * the user's language and the message stays a developer-facing line. */
export class TorrentClientFailure extends Error {
  public constructor(public readonly failure: TorrentClientError) {
    super(describeFailure(failure));
    this.name = 'TorrentClientFailure';
  }
}

/** Anything a request can throw, as the sealed error. A rejection that is not
 * one of ours came out of `fetch` itself (DNS, refused connection, the 10 s
 * abort), and all of those are the same thing to whoever has to fix it. */
export function toTorrentClientError(cause: unknown): TorrentClientError {
  return cause instanceof TorrentClientFailure ? cause.failure : { kind: 'unreachable' };
}

function describeFailure(failure: TorrentClientError): string {
  switch (failure.kind) {
    case 'login-refused':
      return 'the torrent client refused the login';
    case 'bad-response':
      return `the torrent client answered HTTP ${failure.status}`;
    case 'rejected':
      return `the torrent client rejected the change: ${failure.detail}`;
    default:
      return 'the torrent client did not answer';
  }
}

/**
 * The base URL of a torrent client's web interface, with any trailing slash
 * removed so a path can be appended to it unconditionally.
 *
 * Refuses anything that is not an absolute `http:` or `https:` URL, and does
 * it before a socket is opened: a typed address is the one field of this
 * feature a user is likely to get wrong, and "cannot reach it" is the honest
 * answer for a string nothing could ever be reached at.
 */
export function torrentClientBaseUrl(raw: string): string {
  let parsed: URL;
  try {
    parsed = new URL(raw.trim());
  } catch {
    throw new TorrentClientFailure({ kind: 'unreachable' });
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new TorrentClientFailure({ kind: 'unreachable' });
  }
  return `${parsed.origin}${parsed.pathname}`.replace(/\/+$/, '');
}

/** Replaces every occurrence of the values the app sent the client with a
 * mask. A client that quotes the request it refused would otherwise put the
 * web interface password on screen, and into whatever the user pastes into a
 * bug report. */
export function redactCredentials(detail: string, secrets: string[]): string {
  return secrets.reduce(
    (text, secret) => (secret.length === 0 ? text : text.split(secret).join('***')),
    detail,
  );
}

/** The client's refusal, compacted to one line, capped and redacted. */
export async function detailFrom(response: Response, secrets: string[]): Promise<string> {
  let body = '';
  try {
    body = await response.text();
  } catch {
    body = '';
  }
  return redactCredentials(body.replace(/\s+/g, ' ').trim().slice(0, MAX_DETAIL_CHARS), secrets);
}

export interface TorrentClientRequest {
  method: 'GET' | 'POST';
  path?: string;
  headers?: Record<string, string>;
  body?: string;
}

/**
 * The one place a torrent client is spoken to over HTTP.
 *
 * Carries the session cookie the client handed back at login, which is how
 * all three of them keep a session, and turns any transport failure into the
 * sealed `unreachable`. The `fetch` is injected so the adapters are testable
 * without a socket, and so the main process keeps a single outbound call site.
 */
export class TorrentClientHttp {
  private readonly cookies = new Map<string, string>();

  public constructor(
    private readonly base: string,
    private readonly fetchImpl: typeof fetch,
  ) {}

  public async send(request: TorrentClientRequest): Promise<Response> {
    const headers = new Headers(request.headers);
    const cookie = this.cookieHeader();
    if (cookie !== undefined) {
      headers.set('cookie', cookie);
    }

    let response: Response;
    try {
      response = await this.fetchImpl(`${this.base}${request.path ?? ''}`, {
        method: request.method,
        headers,
        body: request.body,
        signal: AbortSignal.timeout(TORRENT_CLIENT_TIMEOUT_MS),
      });
    } catch {
      throw new TorrentClientFailure({ kind: 'unreachable' });
    }

    this.harvestCookies(response);
    return response;
  }

  private cookieHeader(): string | undefined {
    if (this.cookies.size === 0) {
      return undefined;
    }
    return [...this.cookies.entries()].map(([name, value]) => `${name}=${value}`).join('; ');
  }

  private harvestCookies(response: Response) {
    const raw =
      typeof response.headers.getSetCookie === 'function'
        ? response.headers.getSetCookie()
        : [response.headers.get('set-cookie') ?? ''];
    for (const entry of raw) {
      const [pair] = entry.split(';');
      const separator = pair.indexOf('=');
      if (separator > 0) {
        this.cookies.set(pair.slice(0, separator).trim(), pair.slice(separator + 1).trim());
      }
    }
  }
}

/** Parses a JSON body, or reports the answer as unreadable rather than
 * letting a syntax error escape as an unknown exception. */
export async function readJsonBody(response: Response): Promise<Record<string, unknown>> {
  try {
    return (await response.json()) as Record<string, unknown>;
  } catch {
    throw new TorrentClientFailure({ kind: 'bad-response', status: response.status });
  }
}

export interface TorrentClientAdapterOptions {
  kind: TorrentClientKind;
  url: string;
  username: string;
  password: string;
  fetch: typeof fetch;
}

/** What the app asks a torrent client to do: say who it is, and listen on the
 * port the exit granted. Nothing else, so the app never needs a permission it
 * does not use. */
export interface TorrentClientAdapter {
  probe(): Promise<ProbeResult>;
  setListenPort(port: number): Promise<void>;
}
