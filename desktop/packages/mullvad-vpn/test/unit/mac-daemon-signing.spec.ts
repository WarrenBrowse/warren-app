import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

// Building the packaging config shells out to `cargo run --bin mullvad-version`,
// which a Node-only CI runner cannot do, so it is answered by a stand-in on PATH
// (see product-env-icons.spec.ts).
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

async function distribution() {
  vi.resetModules();
  return import('../../tasks/distribution.cjs');
}

const APP = '/tmp/build/Warren VPN.app';
const DEFAULTS = {
  entitlements: '/electron/entitlements.mac.plist',
  hardenedRuntime: true,
  timestamp: undefined,
  additionalArguments: [],
};

describe('macOS signing of the daemon', () => {
  const timeoutMs = 60_000;

  it(
    'signs the daemon with its own entitlements and keeps the hardened runtime',
    async () => {
      const { macSignOptionsForFile, MAC_DAEMON_ENTITLEMENTS } = await distribution();

      const options = macSignOptionsForFile(`${APP}/Contents/Resources/warren-daemon`, DEFAULTS);

      expect(options).toEqual({ ...DEFAULTS, entitlements: MAC_DAEMON_ENTITLEMENTS });
    },
    timeoutMs,
  );

  it(
    'leaves every other binary on the entitlements Electron needs',
    async () => {
      const { macSignOptionsForFile } = await distribution();

      for (const file of [
        APP,
        `${APP}/Contents/Frameworks/Warren VPN Helper (Renderer).app`,
        `${APP}/Contents/Resources/warren`,
      ]) {
        expect(macSignOptionsForFile(file, DEFAULTS), file).toBe(DEFAULTS);
      }
    },
    timeoutMs,
  );

  // Every entitlement a hardened daemon carries is an exception to the
  // hardened runtime; it needs none.
  it(
    'grants the daemon no hardened runtime exception',
    async () => {
      const { MAC_DAEMON_ENTITLEMENTS } = await distribution();

      const plist = fs.readFileSync(MAC_DAEMON_ENTITLEMENTS, 'utf8');

      expect(plist).toContain('<dict/>');
      expect(plist).not.toContain('<key>');
    },
    timeoutMs,
  );

  it(
    'routes the app signature through the daemon-aware signer',
    async () => {
      const { newConfig } = await distribution();

      const config = newConfig();

      expect(typeof config.mac.sign).toBe('function');
    },
    timeoutMs,
  );
});
