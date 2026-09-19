import { NatPmpProto } from './daemon-rpc-types';

/**
 * The torrent clients the app can drive through their own web API.
 *
 * Each one exposes a local HTTP interface the user already has to enable to
 * use its web UI, so nothing has to be installed beside it: the app logs in
 * the same way a browser would and writes the incoming port the exit granted.
 */
export type TorrentClientKind = 'qbittorrent' | 'transmission' | 'deluge';

/** Every `TorrentClientKind`, in the order the settings select offers them. */
export const TORRENT_CLIENT_KINDS: TorrentClientKind[] = ['qbittorrent', 'transmission', 'deluge'];

/** What a client answers when asked who it is and what it listens on. Both
 * fields are optional because a client may answer one and not the other (a
 * Deluge web interface with no daemon attached knows its own version and no
 * port at all). */
export interface ProbeResult {
  version: string | undefined;
  listenPort: number | undefined;
}

/**
 * Why talking to a torrent client failed.
 *
 * Sealed rather than a message string so the renderer can phrase each case in
 * the user's language, and so no free-form text from the client can carry a
 * credential into the UI. `detail` is the client's own refusal, already
 * redacted of anything the app sent it.
 */
export type TorrentClientError =
  | { kind: 'unreachable' }
  | { kind: 'login-refused' }
  | { kind: 'bad-response'; status: number }
  | { kind: 'rejected'; detail: string };

/** The NAT-PMP rule a torrent client is bound to, by the exit-side identity
 * `(internalPort, protocol)` the daemon stores its mappings under. */
export interface TorrentClientRuleRef {
  internalPort: number;
  protocol: NatPmpProto;
}

/** The torrent client configuration as the main process uses it. The password
 * is NOT part of it: it is fetched separately from the sealed store, so a
 * config object can be logged or passed around without carrying one. */
export interface TorrentClientConfig {
  kind: TorrentClientKind | 'none';
  url: string;
  username: string;
  rule?: TorrentClientRuleRef;
}

/** The only shape of the configuration the renderer ever sees. `hasPassword`
 * replaces the password itself, which never crosses the IPC boundary in
 * either direction except as a fresh value the user just typed. */
export interface TorrentClientPublicConfig {
  kind: TorrentClientKind | 'none';
  url: string;
  username: string;
  hasPassword: boolean;
  rule?: TorrentClientRuleRef;
}

/**
 * A configuration change coming from the settings form.
 *
 * An absent `password` keeps the stored one, which is what lets the form
 * render an empty password field over a saved password without erasing it on
 * the next save; an empty string clears it.
 */
export interface TorrentClientConfigUpdate {
  kind: TorrentClientKind | 'none';
  url: string;
  username: string;
  password?: string;
  rule?: TorrentClientRuleRef;
}

/** Why a configuration change was refused. Sealed for the same reason as
 * {@link TorrentClientError}. */
export interface TorrentClientConfigError {
  error: 'encryption-unavailable';
}

/** What `setConfig` answers: the stored configuration as the renderer may see
 * it, or the reason it was not stored. */
export type TorrentClientConfigResult = TorrentClientPublicConfig | TorrentClientConfigError;

/** Whether a `setConfig` answer is a refusal. */
export function isTorrentClientConfigError(
  result: TorrentClientConfigResult,
): result is TorrentClientConfigError {
  return 'error' in result;
}

/**
 * What the app is currently doing with the torrent client.
 *
 * `waiting` means the exit holds no public port for the linked rule, so there
 * is nothing better to write: the client keeps the port it already has rather
 * than being pointed at a dead one.
 */
export interface TorrentClientStatus {
  state: 'off' | 'waiting' | 'pushing' | 'synced' | 'error';
  port?: number;
  version?: string;
  error?: TorrentClientError;
  at: number;
}

/** The client's own name, as its project spells it. Never translated: these
 * are product names. */
export function torrentClientLabel(kind: TorrentClientKind | 'none'): string {
  switch (kind) {
    case 'qbittorrent':
      return 'qBittorrent';
    case 'transmission':
      return 'Transmission';
    case 'deluge':
      return 'Deluge';
    default:
      return '';
  }
}

/** The address a fresh install of each client serves its web interface on.
 * Shown as the placeholder of the address field so the common case is one
 * click away. */
export function torrentClientDefaultUrl(kind: TorrentClientKind | 'none'): string {
  switch (kind) {
    case 'qbittorrent':
      return 'http://127.0.0.1:8080';
    case 'transmission':
      return 'http://127.0.0.1:9091';
    case 'deluge':
      return 'http://127.0.0.1:8112';
    default:
      return '';
  }
}

/** Deluge authenticates on a password alone: its web interface has no user
 * name, so the form hides the field rather than asking for something the
 * client will ignore. */
export function torrentClientUsesUsername(kind: TorrentClientKind | 'none'): boolean {
  return kind === 'qbittorrent' || kind === 'transmission';
}

/**
 * The torrent client configuration as it sits in `gui_settings.json`.
 *
 * The password is the only sealed value in that file, which is cleartext by
 * invariant everywhere else: it is a credential to a service on the user's own
 * machine, so it is stored the way the forum identity is, through the OS
 * keychain, and what remains readable in the file says only which client is
 * configured and where.
 */
export interface StoredTorrentClient {
  kind: TorrentClientKind | 'none';
  url: string;
  username: string;
  /** The password, sealed by the platform secret store and base64-encoded.
   * Empty when no password is stored. */
  passwordEncrypted: string;
  rule?: TorrentClientRuleRef;
}

function parseRuleRef(value: unknown): TorrentClientRuleRef | undefined {
  if (typeof value !== 'object' || value === null) {
    return undefined;
  }
  const raw = value as Record<string, unknown>;
  const internalPort = raw['internalPort'];
  const protocol = raw['protocol'];
  if (typeof internalPort !== 'number' || !Number.isInteger(internalPort)) {
    return undefined;
  }
  if (!Object.values(NatPmpProto).includes(protocol as NatPmpProto)) {
    return undefined;
  }
  return { internalPort, protocol: protocol as NatPmpProto };
}

/**
 * The stored configuration, or `undefined` for anything the app did not
 * write.
 *
 * `gui_settings.json` is a file on disk that a user may edit, and a settings
 * file from a future version may carry a client this build does not speak. A
 * blob that does not check out is dropped whole rather than half-used; a rule
 * that does not check out is dropped on its own, because losing the link to a
 * port is not a reason to lose the credentials with it.
 */
export function parseStoredTorrentClient(value: unknown): StoredTorrentClient | undefined {
  if (typeof value !== 'object' || value === null) {
    return undefined;
  }
  const raw = value as Record<string, unknown>;
  const kind = raw['kind'];
  if (kind !== 'none' && !TORRENT_CLIENT_KINDS.includes(kind as TorrentClientKind)) {
    return undefined;
  }
  const url = raw['url'];
  const username = raw['username'];
  const passwordEncrypted = raw['passwordEncrypted'] ?? '';
  if (
    typeof url !== 'string' ||
    typeof username !== 'string' ||
    typeof passwordEncrypted !== 'string'
  ) {
    return undefined;
  }
  return {
    kind: kind as TorrentClientKind | 'none',
    url,
    username,
    passwordEncrypted,
    rule: parseRuleRef(raw['rule']),
  };
}
