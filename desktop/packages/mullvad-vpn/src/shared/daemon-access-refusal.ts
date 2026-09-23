/**
 * Why the daemon refuses this account, as far as the app words it:
 * - `ownedByAnotherAccount`: another account set Warren up here, and only it or
 *   an administrator may use it;
 * - `claimNeedsConsoleUser`: Warren was set up before it recorded an owner, and
 *   only the account at the computer's own screen may take it over.
 */
export type DaemonAccessRefusal = 'ownedByAnotherAccount' | 'claimNeedsConsoleUser';
