// Whether the user interface settings offer the detached window switch. Linux has no
// overlay to go back to. macOS offers it only to development builds, yet a development
// build shares its settings with the installed app, so a detached window switched on there
// keeps the switch reachable to turn it back off.
export function showUnpinnedWindowSetting(
  platform: NodeJS.Platform,
  development: boolean,
  unpinnedWindow: boolean,
): boolean {
  switch (platform) {
    case 'win32':
      return true;
    case 'darwin':
      return development || unpinnedWindow;
    default:
      return false;
  }
}
