import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

// Building the packaging config resolves the product version by shelling out to
// `cargo run --bin mullvad-version`. The icon paths under test do not depend on
// that version, and the machines that run this suite are not all Rust machines
// (the desktop CI job installs Node and nothing else), so the subprocess is
// answered by a stand-in on PATH. A CommonJS `require` inside `distribution.cjs`
// escapes module mocking, which is why the boundary is stubbed here rather than
// with `vi.mock`.
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
async function iconsFor(productEnv: string) {
  vi.resetModules();
  process.env.WARREN_PRODUCT_ENV = productEnv;
  const distribution = await import('../../tasks/distribution.cjs');
  const config = distribution.newConfig();
  return {
    macos: config.mac.icon,
    linux: config.linux.icon,
    windows: config.win.icon,
    installerSidebar: config.nsis.installerSidebar,
  };
}

describe('per-environment app icons', () => {
  afterEach(() => {
    delete process.env.WARREN_PRODUCT_ENV;
  });

  // Loading the packaging config three times over is slow enough to blow the
  // default 5s budget.
  const timeoutMs = 60_000;

  it(
    'points every platform at an icon that exists on disk',
    async () => {
      for (const productEnv of ['prod', 'beta', 'staging']) {
        const icons = await iconsFor(productEnv);
        for (const [platform, iconPath] of Object.entries(icons)) {
          expect(fs.existsSync(iconPath), `${productEnv} ${platform}: ${iconPath}`).toBe(true);
        }
      }
    },
    timeoutMs,
  );

  // A non-prod install has to be tellable from a prod one on the desktop, in
  // the dock and in the installer, so it may never reuse a prod icon asset.
  it(
    'gives a non-prod build its own icon on every platform',
    async () => {
      const prod = await iconsFor('prod');
      for (const productEnv of ['beta', 'staging']) {
        const icons = await iconsFor(productEnv);
        for (const platform of Object.keys(prod) as (keyof typeof prod)[]) {
          expect(icons[platform], `${productEnv} ${platform}`).not.toBe(prod[platform]);
        }
      }
    },
    timeoutMs,
  );
});
