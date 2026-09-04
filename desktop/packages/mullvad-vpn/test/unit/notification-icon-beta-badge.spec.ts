import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import { importForProductEnv } from './client-rules';

type NotificationControllerModule = typeof import('../../src/main/notification-controller');

// The resolver anchors on `import.meta.dirname`, which points at src/main in a
// source tree and at build/ in a packaged app, so the suite checks the segment
// the resolver appends against the real asset directory. Same shape as
// tray-icon-beta-palette.spec.ts, which pins the tray tree the same way.
const ASSETS_DIR = path.resolve(__dirname, '../../assets/images');

const BADGED_DIR = 'beta';

async function iconPathFor(productEnv: string): Promise<string> {
  const { notificationIconRelativePath } = await importForProductEnv<NotificationControllerModule>(
    productEnv,
    '../../src/main/notification-controller',
  );
  return notificationIconRelativePath.split(path.sep).join('/');
}

describe('the system-notification icon', () => {
  it('carries the production mark only in a production build', async () => {
    expect(await iconPathFor('prod')).to.equal('assets/images/icon-notification.png');
  });

  it('serves the badged copy to every non-prod build', async () => {
    // Linux and Windows draw this icon into every system notification the app
    // raises, so a beta build reading the prod file puts the production mark on
    // the desktop of a tester running both installs. Staging shares the beta
    // artwork, exactly as the tray tree and the app icon do: what matters is
    // being tellable from prod, and there is no third palette.
    for (const environment of ['beta', 'staging']) {
      expect(await iconPathFor(environment), environment).to.equal(
        `assets/images/${BADGED_DIR}/icon-notification.png`,
      );
    }
  });

  it('resolves to a file that exists for every environment', async () => {
    for (const environment of ['prod', 'staging', 'beta']) {
      const relative = (await iconPathFor(environment)).replace('assets/images/', '');
      expect(fs.existsSync(path.join(ASSETS_DIR, relative)), environment).to.be.true;
    }
  });
});
