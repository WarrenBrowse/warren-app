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

const HEADING = /^#+\s+/;
const LIST_ITEM = /^-\s+/;

/**
 * Reflows the licence for a narrow window. The file is hard-wrapped Markdown:
 * shown as is, every wrapped line breaks again inside the view. Paragraphs and
 * list items become single lines, headings lose their markers, and blank lines
 * stay paragraph breaks.
 */
export function formatLicenseText(markdown: string): string {
  return markdown
    .split(/\n\s*\n/)
    .map((block) => {
      const lines: string[] = [];
      for (const raw of block.split('\n')) {
        const line = raw.trim();
        if (line === '') {
          continue;
        }
        if (HEADING.test(line) || LIST_ITEM.test(line) || lines.length === 0) {
          lines.push(line.replace(HEADING, '').replace(LIST_ITEM, ''));
        } else {
          lines[lines.length - 1] += ` ${line}`;
        }
      }
      return lines.join('\n');
    })
    .filter((block) => block !== '')
    .join('\n\n');
}

/**
 * The GPL text shipped with the app, for the licences screen. An empty string
 * when the file cannot be read: the screen still names the licence and links
 * to its source, which beats no screen at all.
 */
export function readLicenseText(location: LicenseLocation): string {
  try {
    return formatLicenseText(fs.readFileSync(licenseFilePath(location), 'utf8'));
  } catch (e) {
    log.error(`Failed to read ${LICENSE_FILE}`, (e as Error).message);
    return '';
  }
}
