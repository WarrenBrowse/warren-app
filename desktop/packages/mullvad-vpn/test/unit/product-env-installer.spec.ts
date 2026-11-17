import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

/**
 * The NSIS include each product environment's Windows installer is built
 * with, read from the resolved packaging config. NSIS cannot run here, so the
 * script text is what is checked: the names it acts on must be the ones this
 * product actually installs, never Mullvad's.
 */
let stubBinDir: string | undefined;
let realPath: string | undefined;

// The packaging config asks `cargo run --bin mullvad-version` for the version,
// which the include does not depend on; a stand-in answers it, as in the other
// product-env specs.
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

afterEach(() => {
  delete process.env.WARREN_PRODUCT_ENV;
});

const repoRoot = path.resolve(__dirname, '../../../../..');

async function installerScript(productEnv: string): Promise<string> {
  vi.resetModules();
  process.env.WARREN_PRODUCT_ENV = productEnv;
  const distribution = await import('../../tasks/distribution.cjs');
  const config = distribution.newConfig() as unknown as { nsis: { include: string } };
  return fs.readFileSync(config.nsis.include, 'utf8');
}

// The salt winfw is compiled with, read from the native module build so the
// installer can never be checked against a second copy of the table.
function winfwSalt(productEnv: string): number {
  const script = fs.readFileSync(path.join(repoRoot, 'build-windows-modules.sh'), 'utf8');
  const match = new RegExp(`^\\s*${productEnv}\\)\\s+echo "(0x[0-9A-Fa-f]+)"`, 'm').exec(script);
  if (match === null) {
    throw new Error(`no winfw salt for ${productEnv} in build-windows-modules.sh`);
  }
  return Number.parseInt(match[1], 16);
}

// winfw salts a GUID by XORing its first field (WarrenEnvGuid in mullvadguids.h).
function salted(guid: string, salt: number): string {
  const [first, ...rest] = guid.split('-');
  const data1 = ((Number.parseInt(first, 16) ^ salt) >>> 0).toString(16).padStart(8, '0');
  return [data1, ...rest].join('-');
}

// NSIS keeps the first definition of a symbol; the include's own are guarded.
function effectiveDefine(script: string, name: string): string | undefined {
  return new RegExp(`^!define ${name} "\\{([0-9a-f-]+)\\}"`, 'm').exec(script)?.[1];
}

const FILTERS = {
  BLOCK_OUTBOUND_IPV4_FILTER_GUID: 'a81c5411-0fd0-43a9-a9be-313f299de64f',
  PERSISTENT_BLOCK_OUTBOUND_IPV4_FILTER_GUID: '79860c64-9a5e-48a3-b5f3-d64b41659aa5',
};

describe('the Windows installer include', () => {
  const timeoutMs = 60_000;

  for (const productEnv of ['prod', 'beta', 'staging']) {
    it(
      `warns about the ${productEnv} build's own firewall filters`,
      async () => {
        const script = await installerScript(productEnv);
        const salt = winfwSalt(productEnv);

        for (const [name, guid] of Object.entries(FILTERS)) {
          expect(effectiveDefine(script, name), name).toBe(salted(guid, salt));
        }
      },
      timeoutMs,
    );
  }

  it(
    'cleans up the names this product installs, not Mullvad ones',
    async () => {
      const script = await installerScript('beta');

      expect(script).not.toContain('mullvad-vpn-updater');
      expect(script).toContain('RMDir /r "$LOCALAPPDATA\\${APP_PACKAGE_NAME}-updater"');
      expect(script).not.toMatch(/Delete "\$INSTDIR\\warren-vpn(-beta)?\.exe"/);
      expect(script).toContain('Delete "$INSTDIR\\${APP_EXECUTABLE_FILENAME}"');
      expect(script).not.toContain('8fa2c331');
    },
    timeoutMs,
  );
});
