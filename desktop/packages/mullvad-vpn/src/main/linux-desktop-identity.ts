import { desktopEntryFileName } from './autostart';

/**
 * Pairs the running window with the desktop entry the package installed.
 *
 * On X11 GNOME matches a window to its launcher through WM_CLASS, which
 * Electron sets to the product name, the same string electron-builder writes
 * as `StartupWMClass`. On a Wayland session the match goes through the
 * toplevel's application id instead, and Chromium derives that id from
 * `CHROME_DESKTOP`, the name of the desktop entry, read from the process
 * environment when the window is created. Unset, the id is empty, GNOME
 * finds no entry to pair the window with, and the dock and the switcher
 * paint the generic icon (Ubuntu 26.04, 1.1.23). Electron's own
 * `app.setDesktopName` writes this same variable and is no longer in its
 * typings, so the variable is set directly.
 */
export function pairWindowWithDesktopEntry(
  env: NodeJS.ProcessEnv,
  platform: NodeJS.Platform = process.platform,
): void {
  if (platform !== 'linux') {
    return;
  }
  env.CHROME_DESKTOP = desktopEntryFileName;
}
