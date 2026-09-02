import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

// Same stand-in as product-env-icons.spec.ts: building the packaging config
// shells out to `cargo run --bin mullvad-version`, and the desktop CI runner has
// Node and nothing else.
let stubBinDir: string | undefined;
let realPath: string | undefined;

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

// The packaging config reads WARREN_PRODUCT_ENV once, at module load, so each
// environment needs a fresh module instance.
async function uninstallScriptFor(productEnv: string) {
  vi.resetModules();
  process.env.WARREN_PRODUCT_ENV = productEnv;
  const distribution = await import('../../tasks/distribution.cjs');
  const config = distribution.newConfig() as unknown as {
    mac: { extraResources: { from: string; to: string }[] };
  };
  const entry = config.mac.extraResources.find((resource) => resource.to === './uninstall.sh');
  expect(entry, `${productEnv}: no uninstall.sh in mac.extraResources`).toBeDefined();
  return fs.readFileSync(entry!.from, 'utf8');
}

// Every name the prod install owns. A non-prod uninstaller that still carries
// one of these deletes the OTHER product on a machine that has both, and leaves
// its own leftovers behind.
const PROD_NAMES: [string, RegExp][] = [
  ['app bundle / GUI process', /Warren(?:\\x20|\\ | )VPN(?!(?:\\x20|\\ | )(?:Beta|Staging))/],
  ['launchd label', /com\.warrenbrowse\.vpn\.daemon/],
  ['CLI symlink', /\/usr\/local\/bin\/warren(?![-\w])/],
  ['zsh completion symlink', /site-functions\/_warren(?![-\w])/],
  ['fish completion symlink', /vendor_completions\.d\/warren\.fish/],
  ['on-disk product dirs', /warren-vpn(?![-\w])/],
];

describe('per-environment macOS uninstaller', () => {
  afterEach(() => {
    delete process.env.WARREN_PRODUCT_ENV;
  });

  // Loading the packaging config several times over is slow enough to blow the
  // default 5s budget.
  const timeoutMs = 60_000;

  it(
    'ships prod the untransformed dist-asset',
    async () => {
      const script = await uninstallScriptFor('prod');
      const source = path.resolve(__dirname, '../../../../../dist-assets/uninstall_macos.sh');
      expect(script).toBe(fs.readFileSync(source, 'utf8'));
    },
    timeoutMs,
  );

  it(
    'never names a prod-installed path in a non-prod uninstaller',
    async () => {
      for (const productEnv of ['beta', 'staging']) {
        const script = await uninstallScriptFor(productEnv);
        for (const [what, pattern] of PROD_NAMES) {
          expect(
            pattern.test(script),
            `${productEnv} uninstaller still targets the prod ${what}`,
          ).toBe(false);
        }
      }
    },
    timeoutMs,
  );

  // The completion links point INTO the app bundle, so one left behind makes
  // every new zsh print a compinit error for a file that no longer exists.
  it(
    'removes the completion symlinks it actually installed',
    async () => {
      for (const productEnv of ['beta', 'staging']) {
        const script = await uninstallScriptFor(productEnv);
        expect(script).toContain(`/usr/local/share/zsh/site-functions/_warren-${productEnv}`);
        expect(script).toContain(`/fish/vendor_completions.d/warren-${productEnv}.fish`);
      }
    },
    timeoutMs,
  );
});
