import fs from 'fs';
import os from 'os';
import path from 'path';
import { renderToStaticMarkup } from 'react-dom/server';
import { ServerStyleSheet } from 'styled-components';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env.platform` at top level.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import { licenseFilePath, readLicenseText } from '../../src/main/license';
import { LicensesContent } from '../../src/renderer/components/views/licenses/LicensesView';
import { appSourceUrl, engineSourceUrl } from '../../src/shared/source-links';

const repoRoot = path.resolve(__dirname, '../../../../..');

function render(element: React.ReactElement): string {
  const sheet = new ServerStyleSheet();
  try {
    return renderToStaticMarkup(sheet.collectStyles(element));
  } finally {
    sheet.seal();
  }
}

// The GPL asks that every copy of the program carries the licence. The
// packaged app ships it next to its other resources, and the licences screen
// reads that same file, so what the user reads is what the installer carries.
describe('the licence the desktop app ships', () => {
  let stubBinDir: string | undefined;
  let realPath: string | undefined;

  // Building the packaging config shells out to `cargo` for the version; see
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

  it('is packaged as a resource of every platform build', async () => {
    vi.resetModules();
    delete process.env.WARREN_PRODUCT_ENV;
    const { newConfig } = await import('../../tasks/distribution.cjs');
    const extraResources = newConfig().extraResources as Array<{ from: string; to: string }>;

    expect(extraResources).toContainEqual({ from: path.join(repoRoot, 'LICENSE.md'), to: '.' });
  }, 60_000);

  it('keeps the upstream copyright beside the fork publisher', async () => {
    vi.resetModules();
    const { newConfig } = await import('../../tasks/distribution.cjs');

    expect(newConfig().copyright).toContain('Mullvad VPN AB');
    expect(newConfig().copyright).toContain('warrenBrowse SRL');
  }, 60_000);

  it('is read from the resources directory of a packaged app', () => {
    expect(
      licenseFilePath({ packaged: true, resourcesPath: '/opt/app/resources', appDir: '/x/build' }),
    ).toBe(path.join('/opt/app/resources', 'LICENSE.md'));
  });

  it('is read from the repository root in a development build', () => {
    const appDir = path.join(repoRoot, 'desktop', 'packages', 'mullvad-vpn', 'build');

    expect(licenseFilePath({ packaged: false, resourcesPath: '/unused', appDir })).toBe(
      path.join(repoRoot, 'LICENSE.md'),
    );
  });

  it('reads the GPL text', () => {
    const appDir = path.join(repoRoot, 'desktop', 'packages', 'mullvad-vpn', 'build');
    const text = readLicenseText({ packaged: false, resourcesPath: '/unused', appDir });

    expect(text).toContain('GNU GENERAL PUBLIC LICENSE');
    expect(text).toContain('Version 3, 29 June 2007');
  });

  it('answers an empty text rather than throwing when the file is missing', () => {
    const appDir = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-no-license-'));
    try {
      expect(readLicenseText({ packaged: true, resourcesPath: appDir, appDir })).toBe('');
    } finally {
      fs.rmSync(appDir, { recursive: true, force: true });
    }
  });
});

describe('the source links', () => {
  const commit = '0123456789abcdef0123456789abcdef01234567';

  it('point at the commit the app was built from', () => {
    expect(appSourceUrl(commit)).toBe(`https://github.com/WarrenBrowse/warren-app/tree/${commit}`);
  });

  it('point at the engine revision the app was built against', () => {
    expect(engineSourceUrl(commit)).toBe(
      `https://github.com/WarrenBrowse/warrenguard/tree/${commit}`,
    );
  });

  it('fall back to the repository when the build recorded no revision', () => {
    expect(appSourceUrl('')).toBe('https://github.com/WarrenBrowse/warren-app/');
    expect(appSourceUrl('not a commit')).toBe('https://github.com/WarrenBrowse/warren-app/');
    expect(engineSourceUrl(undefined)).toBe('https://github.com/WarrenBrowse/warrenguard/');
  });
});

describe('the licences screen', () => {
  const html = render(
    <LicensesContent
      licenseText={'GNU GENERAL PUBLIC LICENSE\n\nVersion 3, 29 June 2007'}
      openAppSource={() => {}}
      openEngineSource={() => {}}
    />,
  );

  it('names the licence of the app', () => {
    expect(html).toContain('GNU General Public License, version 3 or any later version');
  });

  it('credits Mullvad VPN AB', () => {
    expect(html).toContain('Mullvad VPN AB');
  });

  it('names the licence of the engine', () => {
    expect(html).toContain('GNU Affero General Public License, version 3');
  });

  it('shows the full licence text', () => {
    expect(html).toContain('Version 3, 29 June 2007');
  });

  it('links to the source of this version and of the engine', () => {
    expect(html).toContain('Source code of this version');
    expect(html).toContain('Source code of the tunnel engine');
  });
});
