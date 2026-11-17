import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

// Loading the packaging module builds no config, but keep the cargo stand-in of
// the other packaging specs so a Node-only runner never reaches a real cargo.
let stubBinDir: string | undefined;
let realPath: string | undefined;
let outDir: string;

beforeAll(() => {
  outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-pkg-scripts-'));
  if (process.platform === 'win32') {
    return;
  }
  stubBinDir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-cargo-stub-'));
  fs.writeFileSync(path.join(stubBinDir, 'cargo'), '#!/bin/sh\necho 1.0.0\n', { mode: 0o755 });
  realPath = process.env.PATH;
  process.env.PATH = `${stubBinDir}${path.delimiter}${realPath ?? ''}`;
});

afterAll(() => {
  fs.rmSync(outDir, { recursive: true, force: true });
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

const POSTINSTALL = path.resolve(__dirname, '../../../../../dist-assets/pkg-scripts/postinstall');
const ARM64_CDHASH = '0123456789abcdef0123456789abcdef01234567';
const X64_CDHASH = 'fedcba9876543210fedcba9876543210fedcba98';
const timeoutMs = 60_000;

describe('launchd spawn constraint of the macOS daemon', () => {
  it(
    'pins a single-architecture daemon to its cdhash',
    async () => {
      const { daemonSpawnConstraint } = await distribution();

      expect(daemonSpawnConstraint([ARM64_CDHASH])).toBe(
        '<key>SpawnConstraint</key><dict><key>cdhash</key>' +
          `<data>${Buffer.from(ARM64_CDHASH, 'hex').toString('base64')}</data></dict>`,
      );
    },
    timeoutMs,
  );

  // The plist sits in an unquoted heredoc of postinstall, so the `$in` operator
  // has to reach bash escaped or it expands to nothing.
  it(
    'accepts any slice of a universal daemon',
    async () => {
      const { daemonSpawnConstraint } = await distribution();

      const data = [ARM64_CDHASH, X64_CDHASH]
        .map((hash) => `<data>${Buffer.from(hash, 'hex').toString('base64')}</data>`)
        .join('');
      expect(daemonSpawnConstraint([ARM64_CDHASH, X64_CDHASH])).toBe(
        '<key>SpawnConstraint</key><dict><key>cdhash</key>' +
          `<dict><key>\\$in</key><array>${data}</array></dict></dict>`,
      );
    },
    timeoutMs,
  );

  // An unsigned build has only an ad-hoc signature, whose cdhash launchd has not
  // been shown to honour; a constraint it refused would leave no daemon at all.
  it(
    'adds no constraint to an unsigned build, without reading the daemon',
    async () => {
      const { daemonSpawnConstraintFor } = await distribution();
      const cdhashesOf = vi.fn(() => [ARM64_CDHASH]);

      expect(daemonSpawnConstraintFor('/nowhere/warren-daemon', false, cdhashesOf)).toBe('');
      expect(cdhashesOf).not.toHaveBeenCalled();
    },
    timeoutMs,
  );

  it(
    'pins a signed build to the cdhash of the daemon it packs',
    async () => {
      const { daemonSpawnConstraint, daemonSpawnConstraintFor } = await distribution();
      const cdhashesOf = vi.fn(() => [ARM64_CDHASH]);

      expect(daemonSpawnConstraintFor('/app/warren-daemon', true, cdhashesOf)).toBe(
        daemonSpawnConstraint([ARM64_CDHASH]),
      );
      expect(cdhashesOf).toHaveBeenCalledWith('/app/warren-daemon');
    },
    timeoutMs,
  );

  it(
    'leaves a placeholder in the launch daemon plist of postinstall',
    async () => {
      const { DAEMON_SPAWN_CONSTRAINT_PLACEHOLDER } = await distribution();
      const postinstall = fs.readFileSync(POSTINSTALL, 'utf8');
      const plist = postinstall.slice(
        postinstall.indexOf('DAEMON_PLIST=$(cat <<-EOM'),
        postinstall.indexOf('</plist>'),
      );

      expect(plist.split(DAEMON_SPAWN_CONSTRAINT_PLACEHOLDER)).toHaveLength(2);
    },
    timeoutMs,
  );

  it(
    'writes the pkg scripts with the constraint in place of the placeholder',
    async () => {
      const { writePkgScripts, DAEMON_SPAWN_CONSTRAINT_PLACEHOLDER } = await distribution();
      const constraint = '<key>SpawnConstraint</key><dict/>';

      writePkgScripts(outDir, constraint);

      const postinstall = fs.readFileSync(path.join(outDir, 'postinstall'), 'utf8');
      expect(postinstall).toContain(constraint);
      expect(postinstall).not.toContain(DAEMON_SPAWN_CONSTRAINT_PLACEHOLDER);
      expect(fs.statSync(path.join(outDir, 'postinstall')).mode & 0o111).not.toBe(0);
      expect(fs.readFileSync(path.join(outDir, 'preinstall'), 'utf8')).toBe(
        fs.readFileSync(path.join(path.dirname(POSTINSTALL), 'preinstall'), 'utf8'),
      );
    },
    timeoutMs,
  );

  it(
    'refuses a pkg whose postinstall still carries the placeholder',
    async () => {
      const { writePkgScripts, assertSpawnConstraintResolved } = await distribution();

      writePkgScripts(outDir, '');
      expect(() => assertSpawnConstraintResolved(outDir)).not.toThrow();

      fs.copyFileSync(POSTINSTALL, path.join(outDir, 'postinstall'));
      expect(() => assertSpawnConstraintResolved(outDir)).toThrow(/spawn constraint/);
    },
    timeoutMs,
  );
});
