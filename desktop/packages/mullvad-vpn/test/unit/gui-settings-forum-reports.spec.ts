import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest';

const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-gui-reports-'));
vi.mock('electron', () => ({ app: { getPath: () => USER_DATA } }));

import GuiSettings from '../../src/main/gui-settings';

const FILE = path.join(USER_DATA, 'gui_settings.json');

describe('the reports this app filed, in the settings file', () => {
  afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));
  beforeEach(() => fs.rmSync(FILE, { force: true }));

  it('survive a restart, newest first', () => {
    const settings = new GuiSettings();
    settings.load();
    settings.addForumReport({ topicId: 1, summary: 'one', createdAt: '2026-11-27T10:00:00.000Z' });
    settings.addForumReport({ topicId: 2, summary: 'two', createdAt: '2026-11-27T11:00:00.000Z' });

    const reloaded = new GuiSettings();
    reloaded.load();
    expect(reloaded.forumReports.map((r) => r.topicId)).toEqual([2, 1]);
  });

  it('drop a damaged entry without resetting every other setting', () => {
    fs.writeFileSync(
      FILE,
      JSON.stringify({
        enableSystemNotifications: false,
        forumReports: [{ topicId: 3, summary: 's', createdAt: '2026-11-27T10:00:00.000Z' }, {}],
      }),
    );
    const settings = new GuiSettings();
    settings.load();
    expect(settings.enableSystemNotifications).toBe(false);
    expect(settings.forumReports.map((r) => r.topicId)).toEqual([3]);
  });
});
