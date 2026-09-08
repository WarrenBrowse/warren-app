import { describe, expect, it } from 'vitest';

import { importForProductEnv, loadClientRules, ProductEnvFixture } from './client-rules';

type LinuxDesktopIdentityModule = typeof import('../../src/main/linux-desktop-identity');

// GNOME pairs a Wayland window with its launcher entry through the toplevel's
// application id, which Chromium derives from the `CHROME_DESKTOP` variable of
// the process that creates the window. The entry is named after the executable
// this environment installs, so the pairing has to follow the environment like
// the autostart symlink does.
const fixture = loadClientRules<ProductEnvFixture>('product_env.json');
const rows = Object.values(fixture.environments);

async function identityFor(productEnv: string) {
  return importForProductEnv<LinuxDesktopIdentityModule>(
    productEnv,
    '../../src/main/linux-desktop-identity',
  );
}

describe('pairing the window with its desktop entry', () => {
  it('names the entry this environment installs, on Linux', async () => {
    for (const row of rows) {
      const identity = await identityFor(row.name);
      const env: NodeJS.ProcessEnv = {};

      identity.pairWindowWithDesktopEntry(env, 'linux');

      expect(env.CHROME_DESKTOP, row.name).to.equal(`${row.unix_product_dir}.desktop`);
    }
  });

  it('gives every environment an entry of its own', async () => {
    const names = new Set<string>();
    for (const row of rows) {
      const identity = await identityFor(row.name);
      const env: NodeJS.ProcessEnv = {};
      identity.pairWindowWithDesktopEntry(env, 'linux');
      names.add(env.CHROME_DESKTOP ?? '');
    }
    expect(names.size, 'two environments would claim one launcher entry').to.equal(rows.length);
  });

  it('leaves the other platforms alone', async () => {
    const identity = await identityFor('beta');
    for (const platform of ['darwin', 'win32'] as const) {
      const env: NodeJS.ProcessEnv = {};

      identity.pairWindowWithDesktopEntry(env, platform);

      expect(env, platform).to.not.have.property('CHROME_DESKTOP');
    }
  });
});
