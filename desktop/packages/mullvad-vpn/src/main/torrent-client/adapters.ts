import { DelugeAdapter } from './deluge';
import { TorrentClientAdapter, TorrentClientAdapterOptions, TorrentClientFailure } from './http';
import { QBittorrentAdapter } from './qbittorrent';
import { TransmissionAdapter } from './transmission';

export { TORRENT_CLIENT_TIMEOUT_MS, TorrentClientFailure, toTorrentClientError } from './http';
export type { TorrentClientAdapter, TorrentClientAdapterOptions } from './http';

/**
 * The adapter for one torrent client.
 *
 * Throws before opening a socket when the address is not an absolute http or
 * https URL, so a typo in the settings form is reported as "cannot reach it"
 * rather than becoming a request to whatever the string happens to resolve
 * to.
 */
export function createTorrentClientAdapter(
  options: TorrentClientAdapterOptions,
): TorrentClientAdapter {
  switch (options.kind) {
    case 'qbittorrent':
      return new QBittorrentAdapter(options);
    case 'transmission':
      return new TransmissionAdapter(options);
    case 'deluge':
      return new DelugeAdapter(options);
    default:
      // A settings file from a future version, or a `none` slipping past a
      // caller's check: refuse rather than hand back `undefined` for the next
      // line to dereference.
      throw new TorrentClientFailure({ kind: 'unreachable' });
  }
}
