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

  // Enable or disable the desktop banner and the tray dot for new activity
  // on the community forum. On by default: the forum has no mail server, so
  // this app is the only way a reply ever reaches its author. The setting is
  // shown only to a wallet that has a forum account, and it gates the badge
  // in neither direction: the bell and its count are app UI, not a
  // notification. Optional so settings files written by older versions keep
  // validating.
  forumNotifications?: boolean;

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

  // Onboarding wizard: true while a wizard run is owed to the user.
  // Set when the daemon confirms a freshly minted identity, and by the
  // Settings "Replay onboarding" CTA; cleared when the wizard is
  // finished or skipped. Persisted so a GUI restart in the middle of the
  // wizard resumes it. The flag is opt-in on purpose: an identity
  // restored from a recovery phrase already owns its wallet and its
  // subscription, so the wizard has nothing to walk it through, and an
  // absent flag (an install predating this setting) means no wizard.
  onboardingPending?: boolean;

  // True between minting a fresh Warren identity and the user confirming
  // they backed up the recovery phrase. The in-session backup gate lives
  // in renderer redux, but that is lost if the GUI restarts mid-backup,
  // which would replay the daemon `logged in` state as a fully logged-in
  // account and strand the user on the main view with an un-backed-up
  // identity. Persisting the flag lets startup re-route to the
  // backup-pending state. Cleared once the backup is confirmed.
  backupPending?: boolean;

  // App-initiated purchases (doc 35) awaiting their webhook voucher,
  // as `${wpid}:${startedUnixMs}` entries. Persisted so a purchase
  // paid after the app was closed is still redeemed on the next run
  // (the server keeps the wpid mapping for 24h). Owned by the main
  // process PurchaseFlow; the renderer never reads it. Optional so
  // settings files written by older versions keep validating.
  pendingPurchases?: Array<string>;
}
