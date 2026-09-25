import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

/**
 * The Linux packaging identity, asserted on the RESOLVED config rather than on
 * the text of `distribution.cjs`.
 *
 * `scripts/dev/smoke-build.sh` used to grep that file for the literals
 * `executableName: 'warren-vpn'` and `/opt/Warren VPN/`. The beta/prod
 * coexistence campaign replaced both with expressions over `productEnv`, which
 * is correct, and the greps went red on working code. A gate that fails on a
 * legitimate refactor gets ignored, and this one was: it sat red through five
 * releases while nobody could tell its noise from a real branding regression.
 *
 * Resolving the config instead pins what actually ships, and it pins it per
 * environment, so a beta package can never take the production name (the
 * mistake that shipped production WFP keys in a beta build once).
 */
let stubBinDir: string | undefined;
let realPath: string | undefined;

// Building the packaging config shells out to `cargo run --bin mullvad-version`
// for the product version. The names under test do not depend on it, and this
// suite runs on a Node-only runner, so the subprocess is answered by a stand-in
// on PATH, exactly as `product-env-icons.spec.ts` does.
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

interface LinuxPackaging {
  executableName: string;
  artifactName: string;
  /** `--directories=` args of the rpm fpm invocation. RPM is the only format
   * here that tracks directory ownership, so it is the only one given them. */
  rpmDirectories: string[];
  /** fpm `<source>=<target>` mappings, per format, for the installed paths. */
  rpmTargets: string[];
  pacmanTargets: string[];
  appId: string;
  productName: string;
}

/**
 * The slice of the electron-builder config this suite reads. `distribution.cjs`
 * is CommonJS with no types, and what TypeScript infers from it is narrower
 * than what it actually returns, so the shape is declared here rather than
 * asserted field by field.
 */
interface DistributionConfig {
  appId: string;
  productName: string;
  linux: { executableName: string; artifactName: string };
  rpm?: { fpm?: unknown };
  pacman?: { fpm?: unknown };
}

// The packaging config reads WARREN_PRODUCT_ENV once, at module load, so each
// environment needs a fresh module instance.
async function packagingFor(productEnv: string): Promise<LinuxPackaging> {
  vi.resetModules();
  process.env.WARREN_PRODUCT_ENV = productEnv;
  const distribution = await import('../../tasks/distribution.cjs');
  const config = distribution.newConfig() as unknown as DistributionConfig;
  const args = (fpm: unknown): string[] =>
    (Array.isArray(fpm) ? fpm : []).filter((arg): arg is string => typeof arg === 'string');
  const directories = (fpm: unknown): string[] =>
    args(fpm)
      .filter((arg) => arg.startsWith('--directories='))
      .map((arg) => arg.slice('--directories='.length));
  // fpm takes installed paths as `<source-on-disk>=<target-on-system>`; the
  // target is what the package actually writes.
  const targets = (fpm: unknown): string[] =>
    args(fpm)
      .filter((arg) => arg.includes('=/'))
      .map((arg) => arg.slice(arg.indexOf('=/') + 1));

  return {
    executableName: config.linux.executableName,
    artifactName: config.linux.artifactName,
    rpmDirectories: directories(config.rpm?.fpm),
    rpmTargets: targets(config.rpm?.fpm),
    pacmanTargets: targets(config.pacman?.fpm),
    appId: config.appId,
    productName: config.productName,
  };
}

describe('Linux packaging identity per product environment', () => {
  afterEach(() => {
    delete process.env.WARREN_PRODUCT_ENV;
  });

  // Loading the packaging config three times over is slow enough to blow the
  // default 5s budget, as in product-env-icons.spec.ts.
  const timeoutMs = 60_000;

  const expected = {
    prod: { executableName: 'warren-vpn', productName: 'Warren VPN', suffix: '' },
    beta: { executableName: 'warren-vpn-beta', productName: 'Warren VPN Beta', suffix: '-beta' },
    staging: {
      executableName: 'warren-vpn-staging',
      productName: 'Warren VPN Staging',
      suffix: '-staging',
    },
  };

  for (const [env, want] of Object.entries(expected)) {
    it(
      `names the ${env} executable and install directory after that environment`,
      async () => {
        const packaging = await packagingFor(env);

        expect(packaging.executableName).toBe(want.executableName);
        expect(packaging.productName).toBe(want.productName);
        // The install directory RPM is told to own. A wrong value here leaves
        // the other environment's tree behind on uninstall, or claims it.
        expect(packaging.rpmDirectories).toEqual([`/opt/${want.productName}/`]);
      },
      timeoutMs,
    );

    it(
      `installs the ${env} binaries and units under their own names`,
      async () => {
        const packaging = await packagingFor(env);

        // Two installed environments coexist on one machine, so every path a
        // package writes carries the environment suffix. A collision here is
        // one install overwriting the other's daemon or systemd unit.
        for (const targets of [packaging.rpmTargets, packaging.pacmanTargets]) {
          expect(targets).toContain(`/usr/bin/warren${want.suffix}`);
          expect(targets).toContain(`/usr/bin/warren-daemon${want.suffix}`);
          expect(targets).toContain(`/usr/lib/systemd/system/warren-daemon${want.suffix}.service`);
        }
      },
      timeoutMs,
    );
  }

  it(
    'never gives a non-production package a production name',
    async () => {
      const beta = await packagingFor('beta');
      const staging = await packagingFor('staging');

      for (const packaging of [beta, staging]) {
        expect(packaging.executableName).not.toBe('warren-vpn');
        expect(packaging.productName).not.toBe('Warren VPN');
        expect(packaging.appId).not.toBe('com.warrenbrowse.vpn');
        expect(packaging.rpmDirectories).not.toContain('/opt/Warren VPN/');
      }
    },
    timeoutMs,
  );

  it(
    'carries no Mullvad branding in any environment',
    async () => {
      for (const env of Object.keys(expected)) {
        const packaging = await packagingFor(env);
        const identity = [
          packaging.executableName,
          packaging.productName,
          packaging.artifactName,
          packaging.appId,
          ...packaging.rpmDirectories,
          ...packaging.rpmTargets,
          ...packaging.pacmanTargets,
        ].join(' ');

        expect(identity.toLowerCase()).not.toContain('mullvad');
        expect(packaging.artifactName).toContain('WarrenVPN');
      }
    },
    timeoutMs,
  );
});

// pacman runs pre_upgrade and post_upgrade on an upgrade, never the install or
// removal scriptlets. With only --before-install and --after-install given to
// fpm, every Arch upgrade, by hand or in-app, left the previous daemon running
// on a deleted binary and the previous GUI open (measured on an Arch Linux ARM
// VM, 2026-09-25).
describe('pacman upgrade scriptlets', () => {
  afterEach(() => {
    delete process.env.WARREN_PRODUCT_ENV;
  });

  async function pacmanFpm(productEnv: string): Promise<string[]> {
    vi.resetModules();
    process.env.WARREN_PRODUCT_ENV = productEnv;
    const distribution = await import('../../tasks/distribution.cjs');
    const config = distribution.newConfig() as unknown as DistributionConfig;
    const fpm = config.pacman?.fpm;
    return (Array.isArray(fpm) ? fpm : []).filter((arg): arg is string => typeof arg === 'string');
  }

  function scriptAfter(fpm: string[], flag: string): string {
    const index = fpm.indexOf(flag);
    expect(index, `${flag} is passed to fpm`).toBeGreaterThanOrEqual(0);
    return fs.readFileSync(fpm[index + 1], 'utf8');
  }

  it('hands the daemon over and closes the GUI before an upgrade', async () => {
    const preUpgrade = scriptAfter(await pacmanFpm('beta'), '--before-upgrade');
    expect(preUpgrade).toContain('pkill -2 -x "warren-gui-beta"');
    expect(preUpgrade).toContain('prepare-restart');
    expect(preUpgrade).toContain('systemctl stop warren-daemon-beta.service');
  }, 60_000);

  it('starts the new daemon after an upgrade', async () => {
    const postUpgrade = scriptAfter(await pacmanFpm('beta'), '--after-upgrade');
    expect(postUpgrade).toContain('systemctl start warren-daemon-beta.service');
  }, 60_000);
});
