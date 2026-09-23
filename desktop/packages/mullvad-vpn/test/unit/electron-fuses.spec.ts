import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

/**
 * The Electron fuses the packaged app is built with. A fuse is burnt into the
 * Electron binary itself, so it holds whatever the environment or the command
 * line of whoever starts the app says: nobody can run the signed binary as a
 * plain Node interpreter, hand it NODE_OPTIONS or an inspector, or have it
 * load code from anywhere but its own, hash-checked app.asar.
 */
let stubBinDir: string | undefined;
let realPath: string | undefined;

// Building the packaging config shells out to `cargo run --bin mullvad-version`
// for the product version, which the fuses do not depend on; this suite runs on
// a Node-only runner, so the subprocess is answered by a stand-in on PATH, as in
// `product-env-icons.spec.ts`.
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

const HARDENED = {
  runAsNode: false,
  enableNodeOptionsEnvironmentVariable: false,
  enableNodeCliInspectArguments: false,
  onlyLoadAppFromAsar: true,
};

async function distribution() {
  vi.resetModules();
  delete process.env.WARREN_PRODUCT_ENV;
  return import('../../tasks/distribution.cjs');
}

describe('the Electron fuses', () => {
  const timeoutMs = 60_000;

  it(
    'are burnt into the macOS and Windows builds, asar integrity included',
    async () => {
      const config = (await distribution()).newConfig() as unknown as {
        electronFuses: Record<string, unknown>;
      };

      expect(config.electronFuses).toEqual({
        ...HARDENED,
        enableEmbeddedAsarIntegrityValidation: true,
        resetAdHocDarwinSignature: true,
      });
    },
    timeoutMs,
  );

  it(
    'are burnt into the Linux binary before the launcher script takes its name',
    async () => {
      const { linuxAfterPack } = await distribution();
      const appOutDir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-linux-pack-'));
      try {
        fs.writeFileSync(path.join(appOutDir, 'warren-vpn'), 'electron');
        fs.writeFileSync(path.join(appOutDir, 'warren-gui-launcher.sh'), 'launcher');
        const flipped: Array<{ binary: string; fuses: unknown }> = [];
        const packager = {
          generateFuseConfig: (fuses: unknown) => ({ generatedFrom: fuses }),
          addElectronFuses: (context: { appOutDir: string }, fuses: unknown) => {
            const binary = fs.readFileSync(path.join(context.appOutDir, 'warren-vpn'), 'utf8');
            flipped.push({ binary, fuses });
            return Promise.resolve();
          },
        };

        await linuxAfterPack(undefined)({ appOutDir, packager });

        expect(flipped).toEqual([{ binary: 'electron', fuses: { generatedFrom: HARDENED } }]);
        expect(fs.readFileSync(path.join(appOutDir, 'warren-gui'), 'utf8')).toBe('electron');
        expect(fs.readFileSync(path.join(appOutDir, 'warren-vpn'), 'utf8')).toBe('launcher');
      } finally {
        fs.rmSync(appOutDir, { recursive: true, force: true });
      }
    },
    timeoutMs,
  );
});
