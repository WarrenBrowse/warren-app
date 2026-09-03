import { execFile } from 'child_process';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { promisify } from 'util';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const execFileAsync = promisify(execFile);

const distributionPath = path.resolve(__dirname, '../../tasks/distribution.cjs');
const stagedLink = path.resolve(
  __dirname,
  '../../../../../build/env-assets-beta/linux/problem-report-link',
);

// The packaging config resolves the product version through `cargo run`, and
// the machine running this suite may have no Rust toolchain; a stand-in on
// PATH answers it, as in the icons spec.
let stubBinDir: string | undefined;

beforeAll(() => {
  if (process.platform === 'win32') {
    return;
  }
  stubBinDir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-cargo-stub-'));
  fs.writeFileSync(path.join(stubBinDir, 'cargo'), '#!/bin/sh\necho 1.0.0\n', { mode: 0o755 });
});

afterAll(() => {
  if (stubBinDir !== undefined) {
    fs.rmSync(stubBinDir, { recursive: true, force: true });
  }
});

describe('the staged problem-report link', () => {
  // vitest runs spec files in parallel workers, and two of them (the icons
  // and the uninstaller specs) load the packaging config for the beta
  // environment, which stages build/env-assets-beta/linux/problem-report-link
  // under the repository. Two workers staging the same path at once raced a
  // remove-then-create pair into EEXIST on the second create, the failure
  // the desktop CI job saw once the second spec landed. Two packagers
  // staging it in a loop for a few hundred milliseconds each collide on
  // every run of that code.
  it.skipIf(process.platform === 'win32')(
    'survives two packagers staging it at once',
    async () => {
      const script = `
        process.env.WARREN_PRODUCT_ENV = 'beta';
        const { envProblemReportLink } = require(${JSON.stringify(distributionPath)});
        const end = Date.now() + 400;
        while (Date.now() < end) {
          envProblemReportLink();
        }
      `;
      const env = {
        ...process.env,
        PATH: `${stubBinDir}${path.delimiter}${process.env.PATH ?? ''}`,
      };
      await Promise.all([0, 1].map(() => execFileAsync(process.execPath, ['-e', script], { env })));

      expect(fs.readlinkSync(stagedLink)).toBe(
        '/opt/Warren VPN Beta/resources/mullvad-problem-report',
      );
    },
    60_000,
  );
});
