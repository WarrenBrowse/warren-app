export interface BlurContext {
  // The pointer sits on the tray icon: that click toggles the window itself.
  cursorOverTray: boolean;
  // A file picker owns the focus; the window comes back when it closes.
  browsingFiles: boolean;
  // The recovery-phrase backup or the onboarding wizard has not finished.
  firstRunPending: boolean;
}

// Whether a pinned (tray-attached) window hides when it loses focus.
//
// It does for daily use. It must not during the first run: the window is
// launched by the installer's finish page, and the focus change that
// follows hid the recovery phrase and the beta activation before the user
// could act on them, leaving a toast that read as a crash (topic 195).
export function shouldHideOnBlur(context: BlurContext): boolean {
  return !context.cursorOverTray && !context.browsingFiles && !context.firstRunPending;
}
