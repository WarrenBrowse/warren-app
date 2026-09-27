import { describe, expect, it } from 'vitest';

import { showUnpinnedWindowSetting } from '../../src/renderer/features/client/utils';

describe('showUnpinnedWindowSetting', () => {
  it('offers the detached window on Windows', () => {
    expect(showUnpinnedWindowSetting('win32', false, false)).toBe(true);
  });

  it('offers it on macOS only to development builds', () => {
    expect(showUnpinnedWindowSetting('darwin', true, false)).toBe(true);
    expect(showUnpinnedWindowSetting('darwin', false, false)).toBe(false);
  });

  // A development build shares its settings with the installed app, so the
  // setting can be on in a build that would not offer it: the switch must stay
  // reachable to turn it back off.
  it('keeps the switch on macOS while the detached window is on', () => {
    expect(showUnpinnedWindowSetting('darwin', false, true)).toBe(true);
  });

  it('never offers it on Linux, where the window is always detached', () => {
    expect(showUnpinnedWindowSetting('linux', false, true)).toBe(false);
  });
});
