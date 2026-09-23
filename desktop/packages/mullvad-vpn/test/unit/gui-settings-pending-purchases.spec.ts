import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, describe, expect, it, vi } from 'vitest';

// GuiSettings resolves its file through `app.getPath`, and the unit suite
// aliases `electron` to an empty module.
const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-gui-purchases-'));
vi.mock('electron', () => ({ app: { getPath: () => USER_DATA } }));

import GuiSettings from '../../src/main/gui-settings';
import { guiSettingsForRenderer } from '../../src/shared/gui-settings-state';

const FILE = path.join(USER_DATA, 'gui_settings.json');

describe('a settings file that still carries pending purchases', () => {
  afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));

  // An earlier build kept each pending purchase here, pull secret included,
  // in a file every program the user runs can read.
  const secret = 'c'.repeat(64);
  const entry = `${'a'.repeat(32)}${secret}:1750000000000:acct1`;

  it('loses them on load, on disk and in what the renderer sees', () => {
    fs.writeFileSync(
      FILE,
      JSON.stringify({ enableSystemNotifications: false, pendingPurchases: [entry] }),
    );

    const settings = new GuiSettings();
    settings.load();

    expect(fs.readFileSync(FILE, 'utf8')).not.toContain(secret);
    expect(JSON.stringify(guiSettingsForRenderer(settings.state))).not.toContain(secret);
    expect(settings.enableSystemNotifications).toBe(false);
  });

  // A setting that fails validation leaves the file as it is; the purchases
  // must go all the same.
  it('loses them even when another setting in the file does not validate', () => {
    fs.writeFileSync(FILE, JSON.stringify({ autoConnect: 'yes', pendingPurchases: [entry] }));

    new GuiSettings().load();

    expect(fs.readFileSync(FILE, 'utf8')).not.toContain(secret);
    expect(JSON.parse(fs.readFileSync(FILE, 'utf8'))).toEqual({ autoConnect: 'yes' });
  });
});
