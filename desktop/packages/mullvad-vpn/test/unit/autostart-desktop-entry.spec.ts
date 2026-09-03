import path from 'path';
import { describe, expect, it } from 'vitest';

import { importForProductEnv, loadClientRules, ProductEnvFixture } from './client-rules';

type AutostartModule = typeof import('../../src/main/autostart');

// Linux autostart is a symlink in ~/.config/autostart pointing at the desktop
// entry the package installed, and electron-builder names that entry after the
// executable (`executableName` in tasks/distribution.cjs, which is the package
// name of the environment). So the entry a build reads, writes and unlinks has
// to be its own: sharing one name means a beta build deleting prod's autostart,
// and looking for a file its own package never installed.
const fixture = loadClientRules<ProductEnvFixture>('product_env.json');
const rows = Object.values(fixture.environments);

async function autostartFor(productEnv: string) {
  return importForProductEnv<AutostartModule>(productEnv, '../../src/main/autostart');
}

describe('the Linux autostart entry', () => {
  it('is named after the executable this environment installs', async () => {
    for (const row of rows) {
      const autostart = await autostartFor(row.name);
      expect(autostart.desktopEntryFileName, row.name).to.equal(`${row.unix_product_dir}.desktop`);
    }
  });

  it('gives every environment an entry of its own', async () => {
    const names = new Set<string>();
    for (const row of rows) {
      names.add((await autostartFor(row.name)).desktopEntryFileName);
    }
    expect(names.size, 'two environments would fight over one autostart symlink').to.equal(
      rows.length,
    );
  });

  it('resolves the symlink inside the autostart directory of the app data dir', async () => {
    const autostart = await autostartFor('beta');
    expect(autostart.autostartEntryPath(path.join('/home', 'tester', '.config'))).to.equal(
      path.join('/home', 'tester', '.config', 'autostart', 'warren-vpn-beta.desktop'),
    );
  });
});
