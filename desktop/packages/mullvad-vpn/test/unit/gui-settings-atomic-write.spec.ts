import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, describe, expect, it, vi } from 'vitest';

// GuiSettings resolves its file through `app.getPath`, and the unit suite
// aliases `electron` to an empty module.
const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-gui-atomic-'));
vi.mock('electron', () => ({ app: { getPath: () => USER_DATA } }));

import { writeFileAtomic } from '../../src/main/atomic-write';
import GuiSettings from '../../src/main/gui-settings';

const FILE = path.join(USER_DATA, 'gui_settings.json');

// The file holds the onboarding and backup gates, the language and the torrent
// client set-up. A write cut short (power loss, a full disk) must leave the
// previous settings readable rather than a truncated file the next launch
// discards for the defaults.
describe('storing the GUI settings', () => {
  afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));

  it('leaves the previous file whole when the write is cut short', () => {
    const target = path.join(USER_DATA, 'cut-short.json');
    fs.writeFileSync(target, '{"previous":true}');
    const cutShort = {
      writeFileSync: (file: string, data: string) => {
        fs.writeFileSync(file, data.slice(0, 5));
        throw Object.assign(new Error('no space left on device'), { code: 'ENOSPC' });
      },
      renameSync: fs.renameSync,
      rmSync: fs.rmSync,
    };

    expect(() => writeFileAtomic(target, '{"next":true}', cutShort)).toThrow();

    expect(fs.readFileSync(target, 'utf8')).toBe('{"previous":true}');
    expect(fs.existsSync(`${target}.tmp`)).toBe(false);
    fs.rmSync(target);
  });

  it('is readable by the user alone, with no temporary file left behind', () => {
    const settings = new GuiSettings();
    settings.load();
    settings.enableSystemNotifications = false;

    if (process.platform !== 'win32') {
      expect(fs.statSync(FILE).mode & 0o777).toBe(0o600);
    }
    expect(fs.readdirSync(USER_DATA)).toEqual(['gui_settings.json']);
    expect(JSON.parse(fs.readFileSync(FILE, 'utf8')).enableSystemNotifications).toBe(false);
  });
});
