import { spawn, spawnSync } from 'child_process';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { describe, expect, it, vi } from 'vitest';

import {
  LinuxUpgradeEffects,
  parseUpgradeStatus,
  RELAUNCHER_SCRIPT,
  runLinuxUpgrade,
} from '../../src/main/linux-app-upgrade';

describe('parseUpgradeStatus', () => {
  it('reads the states the daemon and the upgrade job write', () => {
    expect(parseUpgradeStatus('running\n')).toEqual({ state: 'running' });
    expect(parseUpgradeStatus('exit 0\n')).toEqual({ state: 'exited', code: 0 });
    expect(parseUpgradeStatus('exit 100\n')).toEqual({ state: 'exited', code: 100 });
  });

  it('refuses anything else', () => {
    expect(parseUpgradeStatus(undefined)).toBeUndefined();
    expect(parseUpgradeStatus('')).toBeUndefined();
    expect(parseUpgradeStatus('exit nope')).toBeUndefined();
  });
});

function makeEffects(
  statuses: Array<string | undefined>,
  overrides: Partial<LinuxUpgradeEffects> = {},
): LinuxUpgradeEffects {
  const queue = [...statuses];
  return {
    installUpgrade: vi.fn().mockResolvedValue('/run/warren-vpn-upgrade.status'),
    spawnRelauncher: vi.fn(),
    readStatus: vi.fn(() => Promise.resolve(queue.length > 1 ? queue.shift() : queue[0])),
    sleep: vi.fn().mockResolvedValue(undefined),
    notifyStarted: vi.fn(),
    notifyInstallFailed: vi.fn(),
    notifyStartFailed: vi.fn(),
    ...overrides,
  };
}

describe('runLinuxUpgrade', () => {
  it('arms the relauncher on the job the daemon started, then reports it started', async () => {
    const effects = makeEffects(['running\n', 'exit 0\n']);
    await runLinuxUpgrade(effects);
    expect(effects.spawnRelauncher).toHaveBeenCalledWith('/run/warren-vpn-upgrade.status');
    expect(effects.notifyStarted).toHaveBeenCalledOnce();
    expect(effects.notifyInstallFailed).not.toHaveBeenCalled();
    expect(effects.notifyStartFailed).not.toHaveBeenCalled();
  });

  it('reports a package manager that failed, since the app keeps running on the old version', async () => {
    const effects = makeEffects(['running\n', 'running\n', 'exit 100\n']);
    await runLinuxUpgrade(effects);
    expect(effects.notifyInstallFailed).toHaveBeenCalledOnce();
  });

  it('reports a job the daemon could not start, and arms no relauncher', async () => {
    const effects = makeEffects([], {
      installUpgrade: vi.fn().mockRejectedValue(new Error('no verified installer')),
    });
    await expect(runLinuxUpgrade(effects)).resolves.toBeUndefined();
    expect(effects.notifyStartFailed).toHaveBeenCalledOnce();
    expect(effects.spawnRelauncher).not.toHaveBeenCalled();
    expect(effects.notifyStarted).not.toHaveBeenCalled();
  });

  it('stops following a job whose status file disappeared', async () => {
    const effects = makeEffects(['running\n', undefined]);
    await runLinuxUpgrade(effects);
    expect(effects.notifyInstallFailed).not.toHaveBeenCalled();
    expect(effects.readStatus).toHaveBeenCalledTimes(2);
  });
});

// The relauncher is what brings the app back: the package's own scripts close
// the GUI in the middle of the upgrade, so nothing inside it can do so.
describe('RELAUNCHER_SCRIPT', () => {
  function sandbox() {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-relauncher-'));
    const status = path.join(dir, 'status');
    const marker = path.join(dir, 'launched');
    const launcher = path.join(dir, 'launcher.sh');
    fs.writeFileSync(launcher, `#!/bin/sh\necho "$@" > '${marker}'\n`, { mode: 0o755 });
    return { status, marker, launcher };
  }

  function run(status: string, guiPid: number, launcher: string) {
    return spawnSync(
      '/bin/sh',
      ['-c', RELAUNCHER_SCRIPT, 'relauncher', status, String(guiPid), launcher, '0'],
      {
        timeout: 20_000,
      },
    );
  }

  // A pid that cannot belong to a live process.
  const deadPid = 2 ** 22 + 7;

  it('starts the new app once the upgrade finished and the old one is gone', () => {
    const { status, marker, launcher } = sandbox();
    fs.writeFileSync(status, 'exit 0\n');
    run(status, deadPid, launcher);
    expect(fs.existsSync(marker)).toBe(true);
  });

  it('waits for a running upgrade to finish', async () => {
    const { status, marker, launcher } = sandbox();
    fs.writeFileSync(status, 'running\n');
    const child = spawn('/bin/sh', [
      '-c',
      RELAUNCHER_SCRIPT,
      'relauncher',
      status,
      String(deadPid),
      launcher,
      '0',
    ]);
    const exited = new Promise((resolve) => child.once('exit', resolve));
    await new Promise((resolve) => setTimeout(resolve, 1500));
    expect(fs.existsSync(marker)).toBe(false);
    fs.writeFileSync(status, 'exit 0\n');
    await exited;
    expect(fs.existsSync(marker)).toBe(true);
  });

  it('leaves alone an app the upgrade did not close', () => {
    const { status, marker, launcher } = sandbox();
    fs.writeFileSync(status, 'exit 100\n');
    run(status, process.pid, launcher);
    expect(fs.existsSync(marker)).toBe(false);
  });
});
