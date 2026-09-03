import fs from 'fs';
import path from 'path';
import { describe, expect, it, vi } from 'vitest';

import { importForProductEnv } from './client-rules';

type TrayModule = typeof import('../../src/main/tray');

const { trayCalls } = vi.hoisted(() => ({
  trayCalls: { toolTips: [] as string[] },
}));

// The suite runs on a machine with no Electron binary, so `Tray` is undefined
// in the shared stub. A recording double gives `createTray` something to build.
vi.mock('electron', () => ({
  Tray: class {
    public setToolTip(toolTip: string) {
      trayCalls.toolTips.push(toolTip);
    }
    public setIgnoreDoubleClickEvents() {
      // no-op
    }
  },
  nativeImage: {
    createFromPath: () => ({}),
    createEmpty: () => ({}),
  },
}));

const USER_INTERFACE_SOURCE = path.resolve(__dirname, '../../src/main/user-interface.ts');

describe('the product the tray names', () => {
  it('is the environment display name, so two installs are tellable apart', async () => {
    for (const [productEnv, displayName] of [
      ['prod', 'Warren VPN'],
      ['staging', 'Warren VPN Staging'],
      ['beta', 'Warren VPN Beta'],
    ]) {
      trayCalls.toolTips = [];
      const { createTray } = await importForProductEnv<TrayModule>(
        productEnv,
        '../../src/main/tray',
      );

      createTray();

      expect(trayCalls.toolTips, productEnv).toEqual([displayName]);
    }
  });

  it('is the environment display name on every surface that names the product', () => {
    // These sites sit in a class whose constructor opens a BrowserWindow, so
    // they cannot be reached from a suite that has no Electron. The gate is
    // that none of them still names the prod product. The split-tunneling and
    // dock entries elsewhere in the file are deliberately left alone: they
    // match the packaged executable name, not the display name.
    const source = fs.readFileSync(USER_INTERFACE_SOURCE, 'utf8');

    expect(source).not.toMatch(/^\s*return 'Warren VPN';$/m);
    expect(source).not.toMatch(/mullvadVpn: 'Warren VPN',/);
    // The application menu, which Linux actually renders whenever the window
    // is unpinned and therefore framed.
    expect(source).not.toMatch(/^\s*label: 'Warren VPN',$/m);
    expect(source).toMatch(/^\s*return productAnchors\.displayName;$/m);
    expect(source).toMatch(/mullvadVpn: productAnchors\.displayName,/);
    expect(source.match(/^\s*label: productAnchors\.displayName,$/gm)).toHaveLength(2);
  });
});
