import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { importForProductEnv, loadClientRules, ProductEnvFixture } from './client-rules';

// The per-environment product anchors are spelled four times (the Rust
// crate, product-env.ts, tasks/distribution.cjs, the Android flavors) and
// pinned by one fixture, fixtures/client-rules/product_env.json, which every
// copy replays. This is the desktop reader: the TypeScript table and the
// packaging identity.
const fixture = loadClientRules<ProductEnvFixture>('product_env.json');
const rows = Object.values(fixture.environments);

// Same stand-in as product-env-icons.spec.ts: building the packaging config
// shells out to `cargo run --bin mullvad-version`, and the desktop CI runner
// has Node and nothing else.
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
async function packagingFor(productEnv: string) {
  vi.resetModules();
  process.env.WARREN_PRODUCT_ENV = productEnv;
  const distribution = await import('../../tasks/distribution.cjs');
  return distribution.newConfig() as unknown as {
    appId: string;
    productName: string;
    protocols: { schemes: string[] }[];
    linux: { executableName: string };
  };
}

describe('product_env.json, the desktop reader', () => {
  afterEach(() => {
    delete process.env.WARREN_PRODUCT_ENV;
  });

  it('names exactly the three environments a build can be compiled for', () => {
    expect(Object.keys(fixture.environments).sort()).toEqual(['beta', 'prod', 'staging']);
    for (const row of rows) {
      expect(fixture.environments[row.name]).toBe(row);
    }
  });

  it('pins the TypeScript anchor table row by row', async () => {
    for (const row of rows) {
      const env = await importForProductEnv<
        typeof import('../../src/shared/constants/product-env')
      >(row.name, '../../src/shared/constants/product-env');
      expect(env.productEnvironment, row.name).toBe(row.name);
      // The table carries a trailing slash: consumers append relative paths.
      expect(env.productAnchors.apiBaseUrl, row.name).toBe(`${row.api_url}/`);
      expect(env.productAnchors.displayName, row.name).toBe(row.display_name);
      expect(env.productAnchors.unixProductDir, row.name).toBe(row.unix_product_dir);
      expect(env.productAnchors.deepLinkScheme, row.name).toBe(row.deep_link_scheme);
    }
  });

  // Loading the packaging config three times over is slow enough to blow the
  // default 5s budget.
  it('pins the packaging identity row by row', async () => {
    for (const row of rows) {
      const config = await packagingFor(row.name);
      expect(config.appId, row.name).toBe(row.application_id);
      expect(config.productName, row.name).toBe(row.display_name);
      expect(
        config.protocols.map((protocol) => protocol.schemes),
        row.name,
      ).toEqual([[row.deep_link_scheme]]);
      expect(config.linux.executableName, row.name).toBe(row.unix_product_dir);
    }
  }, 60_000);
});
