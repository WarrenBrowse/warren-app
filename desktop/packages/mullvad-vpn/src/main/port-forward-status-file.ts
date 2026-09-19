import fs from 'fs/promises';
import path from 'path';

import { NatPmpStatus } from '../shared/daemon-rpc-types';

/** Name of the machine-readable status file, next to the GUI settings in the
 * app's user-data directory. A script watches it to learn the public port
 * without talking to the daemon or parsing anything meant for a human. */
export const FORWARDED_PORT_FILE_NAME = 'forwarded_port';

// Written next to the target so the rename that publishes it is atomic: a
// reader either sees the previous content or the new one, never a half-written
// line. A fixed name is enough because the app is single-instance and the main
// process is the only writer.
const TEMPORARY_FILE_NAME = `${FORWARDED_PORT_FILE_NAME}.tmp`;

/**
 * The contents of `forwarded_port` for a NAT-PMP snapshot: one granted public
 * port per line, in snapshot order, each line terminated by a newline.
 *
 * Empty when nothing is granted AND when the tunnel is down. The daemon keeps
 * its mapping list across a disconnect, so a port read from a down tunnel
 * would send a torrent client at an address nothing answers on; the home chip
 * hides for that same reason.
 */
export function renderForwardedPortFile(
  status: NatPmpStatus | undefined,
  tunnelConnected: boolean,
): string {
  if (!tunnelConnected || status === undefined) {
    return '';
  }
  return status.mappings
    .map((mapping) => (mapping.status.state === 'mapped' ? `${mapping.status.externalPort}\n` : ''))
    .join('');
}

/**
 * Publishes `content` as `<directory>/forwarded_port`, atomically.
 *
 * Empty content truncates the file instead of removing it: a watcher follows
 * the inode it opened, so unlinking the file would silently take the watch
 * down with it and the next grant would reach nobody.
 */
export async function writeForwardedPortFile(directory: string, content: string): Promise<void> {
  const temporary = path.join(directory, TEMPORARY_FILE_NAME);
  await fs.writeFile(temporary, content, 'utf8');
  await fs.rename(temporary, path.join(directory, FORWARDED_PORT_FILE_NAME));
}

/**
 * Publishes the status file only when its contents actually change.
 *
 * Every tunnel-state change republishes, and one connect produces several, so
 * a plain write would wake a watcher each time and restart a torrent client on
 * a port that never moved. A failed write is not remembered as published, so
 * the next attempt with the same content still goes through.
 */
export class ForwardedPortFile {
  private lastContent?: string;

  public async publish(directory: string, content: string): Promise<void> {
    if (content === this.lastContent) {
      return;
    }
    await writeForwardedPortFile(directory, content);
    this.lastContent = content;
  }
}
