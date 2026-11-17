import fs from 'fs';
import path from 'path';

import log from '../shared/logging';

const LICENSE_FILE = 'LICENSE.md';

export interface LicenseLocation {
  packaged: boolean;
  // `process.resourcesPath`: packaging copies the licence there on every platform.
  resourcesPath: string;
  // The directory the main bundle runs from (`build/`), four levels below the
  // repository root in a development checkout.
  appDir: string;
}

export function licenseFilePath({ packaged, resourcesPath, appDir }: LicenseLocation): string {
  if (packaged) {
    return path.join(resourcesPath, LICENSE_FILE);
  }
  return path.resolve(appDir, '..', '..', '..', '..', LICENSE_FILE);
}

/**
 * The GPL text shipped with the app, for the licences screen. An empty string
 * when the file cannot be read: the screen still names the licence and links
 * to its source, which beats no screen at all.
 */
export function readLicenseText(location: LicenseLocation): string {
  try {
    return fs.readFileSync(licenseFilePath(location), 'utf8');
  } catch (e) {
    log.error(`Failed to read ${LICENSE_FILE}`, (e as Error).message);
    return '';
  }
}
