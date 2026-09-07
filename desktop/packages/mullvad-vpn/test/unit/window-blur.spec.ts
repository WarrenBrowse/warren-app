import { describe, expect, it } from 'vitest';

import { shouldHideOnBlur } from '../../src/main/window-blur';

// The pinned window hides whenever it loses focus, which is right for daily
// use and wrong for a first run launched from an installer: the recovery
// phrase and the beta activation vanished behind a focus change and the
// user read the resulting toast as a crash (topic 195).
describe('shouldHideOnBlur', () => {
  it('hides the pinned window on an ordinary blur', () => {
    expect(
      shouldHideOnBlur({ cursorOverTray: false, browsingFiles: false, firstRunPending: false }),
    ).toBe(true);
  });

  it('keeps the window while the cursor is over the tray icon, the click toggles it', () => {
    expect(
      shouldHideOnBlur({ cursorOverTray: true, browsingFiles: false, firstRunPending: false }),
    ).toBe(false);
  });

  it('keeps the window while a file picker holds the focus', () => {
    expect(
      shouldHideOnBlur({ cursorOverTray: false, browsingFiles: true, firstRunPending: false }),
    ).toBe(false);
  });

  it('keeps the window while the recovery-phrase backup or the wizard is pending', () => {
    expect(
      shouldHideOnBlur({ cursorOverTray: false, browsingFiles: false, firstRunPending: true }),
    ).toBe(false);
  });
});
