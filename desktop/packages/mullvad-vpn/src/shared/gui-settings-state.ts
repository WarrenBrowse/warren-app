// This is a special value which is when contained within IGuiSettingsState.preferredLocale
// indicates that app should use the active operating system locale to determine the UI language.
export const SYSTEM_PREFERRED_LOCALE_KEY = 'system';

export interface IGuiSettingsState {
  // A user interface locale.
  // Use 'system' to opt-in for active locale set in the operating system
  // (see SYSTEM_PREFERRED_LOCALE_KEY)
  preferredLocale: string;

  // Enable or disable system notifications on tunnel state etc.
  enableSystemNotifications: boolean;

  // Tells the app to activate auto-connect feature in the mullvad-daemon, but only if the app is
  // set to auto-start with the system.
  autoConnect: boolean;

  // Tells the app to use monochromatic set of icons for tray.
  monochromaticIcon: boolean;

  // Tells the app to hide the main window on start.
  startMinimized: boolean;

  // Tells the app whether or not it should act as a window or a context menu.
  unpinnedWindow: boolean;

  // Contains a list of filepaths to applications added to the list of applications, in the split
  // tunneling view, by the user.
  browsedForSplitTunnelingApplications: Array<string>;

  // The last version that the changelog dialog was shown for. This is used to only show the
  // changelog after upgrade.
  changelogDisplayedForVersion: string;

  // The last version that the update dialog was dismissed for. This is used to determine
  // whether to show the update notification.
  updateDismissedForVersion: string;

  // Tells the app whether or not to show the map in the main view.
  animateMap: boolean;

  // M5.B.3 onboarding wizard: timestamp (Unix seconds) at which the
  // wizard was last completed. `undefined` -> first launch, the
  // wizard router intercepts the boot and shows the welcome step.
  // `Some(ts)` -> wizard already gone through; the user can replay
  // it from the Settings "Replay onboarding" CTA, which clears this
  // field. The value lets future versions invalidate the existing
  // completion (e.g., a new wallet model in M6) without breaking the
  // existing user base by simply bumping the cutoff in the renderer
  // boot logic.
  onboardingCompletedUnix?: number;

  // M5.B.2 multi-exit auto-failover toggle. The daemon implements
  // failover unconditionally; this GUI-only flag governs whether the
  // failover notification toast is surfaced and whether the settings
  // panel shows the toggle as ON. Default true (= ON). Persisted in
  // gui_settings.json (no proto field). A future proto field can
  // supersede this once exit-quality telemetry graduates from POC to
  // first-class daemon state.
  warrenFailoverEnabled?: boolean;
}
