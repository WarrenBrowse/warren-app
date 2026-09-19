import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, describe, expect, it, vi } from 'vitest';

// GuiSettings resolves its file through `app.getPath`, and the unit suite
// aliases `electron` to an empty module. A temporary user-data directory is
// enough to exercise the store/load round trip the setting has to survive.
const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-pf-settings-'));
vi.mock('electron', () => ({ app: { getPath: () => USER_DATA } }));

import GuiSettings from '../../src/main/gui-settings';

describe('the port-forwarding notifications setting', () => {
  afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));

  // A user who opened a public port wants to be told when it moves: that is
  // the whole reason the setting exists, so it is on until it is turned off.
  it('is on by default', () => {
    expect(new GuiSettings().portForwardingNotifications).toBe(true);
  });

  it('survives a reload once turned off', () => {
    const settings = new GuiSettings();
    settings.load();
    settings.portForwardingNotifications = false;

    const reloaded = new GuiSettings();
    reloaded.load();

    expect(reloaded.portForwardingNotifications).toBe(false);
  });

  // The key is absent from every settings file written before this lot, and
  // an absent key must not fail validation and reset the whole file.
  it('reads a settings file written before the setting existed', () => {
    fs.writeFileSync(
      path.join(USER_DATA, 'gui_settings.json'),
      JSON.stringify({ enableSystemNotifications: false }),
    );

    const settings = new GuiSettings();
    settings.load();

    expect(settings.portForwardingNotifications).toBe(true);
    expect(settings.enableSystemNotifications).toBe(false);
  });
});
