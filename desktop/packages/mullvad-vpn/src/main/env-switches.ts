type Env = Record<string, string | undefined>;

// The upstream names (`MULLVAD_*`) are still read so existing wrappers keep
// working while they move to the Warren names.

/** `WARREN_DISABLE_UPDATE_NOTIFICATION=1` hides the update notification. */
export function updateNotificationDisabled(env: Env): boolean {
  return (
    env.WARREN_DISABLE_UPDATE_NOTIFICATION === '1' ||
    env.MULLVAD_DISABLE_UPDATE_NOTIFICATION === '1'
  );
}

/** `WARREN_PATH`: where a development build finds the auxiliary tools. */
export function devToolsPath(env: Env): string | undefined {
  return env.WARREN_PATH || env.MULLVAD_PATH || undefined;
}
