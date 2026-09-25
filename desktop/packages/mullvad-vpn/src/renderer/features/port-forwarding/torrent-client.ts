import { sprintf } from 'sprintf-js';

import { messages } from '../../../shared/gettext';
import {
  ProbeResult,
  TorrentClientError,
  torrentClientLabel,
  TorrentClientPublicConfig,
  TorrentClientStatus,
} from '../../../shared/torrent-client';

/**
 * The one line the torrent client section shows under its controls.
 *
 * `undefined` where there is nothing to say: the feature is off, or a write is
 * in the air and the buttons already show they are busy. Pure, so every case
 * of the table is pinned without rendering a component.
 */
export function torrentClientStatusLine(
  status: TorrentClientStatus,
  config: TorrentClientPublicConfig,
): string | undefined {
  switch (status.state) {
    case 'waiting':
      return messages.pgettext('port-forwarding-view', 'Waiting for a public port');
    case 'synced':
      return sprintf(
        // TRANSLATORS: Confirmation that the torrent client took the public
        // TRANSLATORS: port. Available placeholders:
        // TRANSLATORS: %(client)s - the torrent client name, e.g. qBittorrent
        // TRANSLATORS: %(port)d - the public port the exit forwards
        messages.pgettext('port-forwarding-view', '%(client)s now listens on port %(port)d'),
        { client: torrentClientLabel(config.kind), port: status.port },
      );
    case 'error':
      return status.error === undefined ? undefined : errorLine(status.error, config);
    case 'held':
      return sprintf(
        // TRANSLATORS: Shown when the app stopped updating the torrent client
        // TRANSLATORS: because its port was closed after an abuse report; the
        // TRANSLATORS: "Apply now" button resumes. Available placeholders:
        // TRANSLATORS: %(port)d - the forwarded port that was closed
        // TRANSLATORS: %(client)s - the torrent client name, e.g. qBittorrent
        messages.pgettext(
          'port-forwarding-view',
          'Port %(port)d was closed after an abuse report. %(client)s keeps its port until you apply one.',
        ),
        { port: status.port, client: torrentClientLabel(config.kind) },
      );
    default:
      return undefined;
  }
}

function errorLine(error: TorrentClientError, config: TorrentClientPublicConfig): string {
  const client = torrentClientLabel(config.kind);
  switch (error.kind) {
    case 'login-refused':
      return sprintf(
        // TRANSLATORS: Shown when the torrent client rejected the credentials.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(client)s - the torrent client name, e.g. qBittorrent
        messages.pgettext('port-forwarding-view', '%(client)s refused the login'),
        { client },
      );
    case 'rejected':
      return sprintf(
        // TRANSLATORS: Shown when the torrent client answered but would not
        // TRANSLATORS: take the port. Available placeholders:
        // TRANSLATORS: %(client)s - the torrent client name, e.g. qBittorrent
        // TRANSLATORS: %(detail)s - the client's own words, in English
        messages.pgettext('port-forwarding-view', '%(client)s rejected the port: %(detail)s'),
        { client, detail: error.detail },
      );
    default:
      // A status the app cannot use means it found no working web API at that
      // address, which is the same thing to whoever has to fix it as nothing
      // answering at all.
      return sprintf(
        // TRANSLATORS: Shown when the torrent client's web interface did not
        // TRANSLATORS: answer. Available placeholders:
        // TRANSLATORS: %(client)s - the torrent client name, e.g. qBittorrent
        // TRANSLATORS: %(url)s - the web interface address the user entered
        messages.pgettext('port-forwarding-view', 'Cannot reach %(client)s at %(url)s'),
        { client, url: config.url },
      );
  }
}

/** What "Test connection" answers when the client did reply. */
export function torrentClientProbeLine(
  probe: ProbeResult,
  config: TorrentClientPublicConfig,
): string {
  const client = torrentClientLabel(config.kind);
  if (probe.listenPort === undefined) {
    return sprintf(
      // TRANSLATORS: Shown when the torrent client answered but did not say
      // TRANSLATORS: which port it listens on. Available placeholder:
      // TRANSLATORS: %(client)s - the torrent client name, e.g. qBittorrent
      messages.pgettext('port-forwarding-view', 'Connected to %(client)s'),
      { client },
    );
  }
  if (probe.version === undefined) {
    return sprintf(
      messages.pgettext('port-forwarding-view', '%(client)s now listens on port %(port)d'),
      { client, port: probe.listenPort },
    );
  }
  return sprintf(
    // TRANSLATORS: Result of the torrent client connection test. Available
    // TRANSLATORS: placeholders:
    // TRANSLATORS: %(client)s - the torrent client name, e.g. qBittorrent
    // TRANSLATORS: %(version)s - the version the client reported
    // TRANSLATORS: %(port)d - the port the client currently listens on
    messages.pgettext(
      'port-forwarding-view',
      'Connected to %(client)s %(version)s, listening on port %(port)d',
    ),
    { client, version: probe.version, port: probe.listenPort },
  );
}

/** Why the settings were not saved. The one refusal the store can answer, and
 * it is about the machine rather than about what the user typed. */
export function torrentClientConfigErrorLine(): string {
  return messages.pgettext(
    'port-forwarding-view',
    'This system cannot store the password securely',
  );
}
