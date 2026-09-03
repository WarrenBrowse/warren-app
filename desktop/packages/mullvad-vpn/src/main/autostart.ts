import { app } from 'electron';
import fs from 'fs';
import path from 'path';

import { productAnchors } from '../shared/constants/product-env';
import log from '../shared/logging';
import { getDesktopEntries } from './linux-desktop-entry';

// The desktop entry this environment's package installed. electron-builder
// names it after the executable, which is the per-environment package name
// (`executableName` in tasks/distribution.cjs, the same string as
// `unixProductDir`). Sharing one name across environments would have a beta
// build delete prod's autostart symlink and then look for an entry its own
// package never installed.
export const desktopEntryFileName = `${productAnchors.unixProductDir}.desktop`;

// Where that environment's autostart symlink lives, under the Electron
// `appData` directory (~/.config on Linux). One name per environment, so two
// installs cannot fight over a single symlink.
export function autostartEntryPath(appDataDir: string): string {
  return path.join(appDataDir, 'autostart', desktopEntryFileName);
}

export function getOpenAtLogin() {
  if (process.platform === 'linux') {
    try {
      const autostartFilePath = autostartEntryPath(app.getPath('appData'));

      fs.accessSync(autostartFilePath);

      return true;
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to check autostart file: ${error.message}`);
      return false;
    }
  } else {
    return app.getLoginItemSettings().openAtLogin;
  }
}

export async function setOpenAtLogin(openAtLogin: boolean) {
  if (process.platform === 'linux') {
    try {
      const desktopFilePath = await getDesktopEntryPath();
      const autostartDir = path.join(app.getPath('appData'), 'autostart');
      const autostartFilePath = autostartEntryPath(app.getPath('appData'));

      if (openAtLogin) {
        await createDirIfNecessary(autostartDir);
        await fs.promises.symlink(desktopFilePath, autostartFilePath);
      } else {
        await fs.promises.unlink(autostartFilePath);
      }
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to set auto-start: ${error.message}`);
    }
  } else {
    app.setLoginItemSettings({ openAtLogin });
  }
}

async function getDesktopEntryPath(): Promise<string> {
  const entries = await getDesktopEntries();
  const entry = entries.find((entry) => path.parse(entry).base === desktopEntryFileName);
  if (entry) {
    return entry;
  } else {
    throw new Error(`Couldn't find ${desktopEntryFileName}`);
  }
}

const createDirIfNecessary = async (directory: string) => {
  let stat;
  try {
    stat = await fs.promises.stat(directory);
  } catch {
    // Path doesn't exist, so it has to be created
    return fs.promises.mkdir(directory);
  }

  // Is there a file instead of a directory?
  if (!stat.isDirectory()) {
    // Try to remove existing file and replace it with a new directory
    try {
      await fs.promises.unlink(directory);
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to remove path before creating a directory for it: ${error.message}`);
    }

    return fs.promises.mkdir(directory);
  }
};
