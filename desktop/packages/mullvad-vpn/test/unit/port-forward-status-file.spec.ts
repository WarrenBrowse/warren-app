import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, describe, expect, it } from 'vitest';

import {
  FORWARDED_PORT_FILE_NAME,
  ForwardedPortFile,
  renderForwardedPortFile,
  writeForwardedPortFile,
} from '../../src/main/port-forward-status-file';
import { NatPmpMapping, NatPmpProto, NatPmpStatus } from '../../src/shared/daemon-rpc-types';

function mapped(internalPort: number, externalPort: number): NatPmpMapping {
  return {
    internalPort,
    protocol: NatPmpProto.both,
    status: { state: 'mapped', externalPort, lifetimeGrantedSecs: 3599, windowResetSecs: 60 },
  };
}

function status(...mappings: NatPmpMapping[]): NatPmpStatus {
  return { mappings };
}

describe('renderForwardedPortFile', () => {
  it('writes one granted port, terminated by a newline', () => {
    expect(renderForwardedPortFile(status(mapped(58291, 58291)), true)).toBe('58291\n');
  });

  it('writes one line per granted port, in snapshot order', () => {
    expect(renderForwardedPortFile(status(mapped(6881, 40002), mapped(58291, 49152)), true)).toBe(
      '40002\n49152\n',
    );
  });

  it('skips a rule that holds no grant', () => {
    const pending: NatPmpMapping = {
      internalPort: 6881,
      protocol: NatPmpProto.tcp,
      status: { state: 'requesting' },
    };

    expect(renderForwardedPortFile(status(pending, mapped(58291, 58291)), true)).toBe('58291\n');
  });

  // The daemon keeps the mapping list across a disconnect, and a port
  // advertised on a down tunnel is a lie: nothing reaches it. The home chip
  // hides for the same reason.
  it('is empty while the tunnel is down, whatever the snapshot still holds', () => {
    expect(renderForwardedPortFile(status(mapped(58291, 58291)), false)).toBe('');
  });

  it('is empty when nothing is mapped', () => {
    expect(renderForwardedPortFile(status(), true)).toBe('');
    expect(renderForwardedPortFile(undefined, true)).toBe('');
  });
});

describe('writeForwardedPortFile', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-forwarded-port-'));
  const file = path.join(directory, FORWARDED_PORT_FILE_NAME);

  afterAll(() => fs.rmSync(directory, { recursive: true, force: true }));

  it('leaves the file holding exactly the content, and no temporary next to it', async () => {
    await writeForwardedPortFile(directory, '58291\n');

    expect(fs.readFileSync(file, 'utf8')).toBe('58291\n');
    expect(fs.readdirSync(directory)).toEqual([FORWARDED_PORT_FILE_NAME]);
  });

  // A watcher (inotify, ReadDirectoryChangesW) follows the inode it opened, so
  // unlinking the file on the way down would take the watch with it. The file
  // stays, and empties.
  it('truncates to zero bytes rather than removing the file', async () => {
    await writeForwardedPortFile(directory, '58291\n');
    await writeForwardedPortFile(directory, '');

    expect(fs.existsSync(file)).toBe(true);
    expect(fs.statSync(file).size).toBe(0);
  });
});

describe('ForwardedPortFile', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-forwarded-port-publish-'));
  const file = path.join(directory, FORWARDED_PORT_FILE_NAME);

  afterAll(() => fs.rmSync(directory, { recursive: true, force: true }));

  // Every tunnel-state change republishes, and a connect alone produces
  // several. Rewriting identical content would wake the watcher each time and
  // restart a torrent client on a port that never moved, which is exactly what
  // the file exists to avoid.
  it('leaves the file untouched when the content has not changed', async () => {
    const publisher = new ForwardedPortFile();
    await publisher.publish(directory, '58291\n');
    const firstWrite = fs.statSync(file).mtimeMs;

    await new Promise((resolve) => setTimeout(resolve, 10));
    await publisher.publish(directory, '58291\n');

    expect(fs.statSync(file).mtimeMs).toBe(firstWrite);
  });

  it('writes again once the content changes', async () => {
    const publisher = new ForwardedPortFile();
    await publisher.publish(directory, '58291\n');
    await publisher.publish(directory, '49152\n');

    expect(fs.readFileSync(file, 'utf8')).toBe('49152\n');
  });

  // A failed write must not be remembered as published, or the retry with the
  // same content would be skipped and the file would stay stale for good.
  it('publishes again after a write that failed', async () => {
    const publisher = new ForwardedPortFile();
    const missing = path.join(directory, 'absent');

    await expect(publisher.publish(missing, '58291\n')).rejects.toThrow();
    fs.mkdirSync(missing);
    await publisher.publish(missing, '58291\n');

    expect(fs.readFileSync(path.join(missing, FORWARDED_PORT_FILE_NAME), 'utf8')).toBe('58291\n');
  });
});
