import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

// Windows shows CompanyName in the UAC and file-properties dialogs, and store
// reviewers compare it, the iOS organization and the package author with the
// developer account that submits the build. All of them name the legal entity
// under its official casing.
const PUBLISHER = 'warrenBrowse SRL';

const repoRoot = path.resolve(__dirname, '../../../../..');

function read(relative: string): string {
  return fs.readFileSync(path.join(repoRoot, relative), 'utf8');
}

function fieldValues(text: string, pattern: RegExp): string[] {
  return [...text.matchAll(pattern)].map((match) => match[1]);
}

describe('publisher identity', () => {
  it('names the company in the PE resources of every Windows binary we build', () => {
    const winres = [
      'mullvad-cli/Cargo.toml',
      'mullvad-daemon/Cargo.toml',
      'mullvad-problem-report/Cargo.toml',
      'windows-installer/Cargo.toml',
      'installer-downloader/Cargo.toml',
    ];
    for (const manifest of winres) {
      const text = read(manifest);
      for (const value of fieldValues(text, /^CompanyName = "([^"]*)"$/gm)) {
        expect(value, `${manifest} CompanyName`).toBe(PUBLISHER);
      }
      const copyrights = fieldValues(text, /^LegalCopyright = "([^"]*)"$/gm);
      expect(copyrights, `${manifest} LegalCopyright`).not.toHaveLength(0);
      for (const value of copyrights) {
        expect(value, `${manifest} LegalCopyright`).toContain(PUBLISHER);
      }
    }

    const winfw = read('windows/winfw/src/winfw/winfw.rc');
    expect(fieldValues(winfw, /VALUE "CompanyName", "([^"]*)"/g)).toEqual([PUBLISHER]);
    expect(fieldValues(winfw, /VALUE "LegalCopyright", "([^"]*)"/g)[0]).toContain(PUBLISHER);
    expect(winfw).not.toMatch(/Mullvad/);
  });

  it('names the company as author of the desktop app packages', () => {
    for (const manifest of ['desktop/package.json', 'desktop/packages/mullvad-vpn/package.json']) {
      const author = JSON.parse(read(manifest)).author;
      expect(typeof author === 'string' ? author : author.name, manifest).toBe(PUBLISHER);
    }
  });

  it('names the company in the installer copyright and the Linux package metadata', () => {
    expect(read('desktop/packages/mullvad-vpn/tasks/distribution.cjs')).toMatch(
      new RegExp(`copyright: '[^']*${PUBLISHER}'`),
    );

    const daemon = read('mullvad-daemon/Cargo.toml');
    for (const key of ['maintainer', 'copyright', 'vendor']) {
      for (const value of fieldValues(daemon, new RegExp(`^${key} = "([^"]*)"$`, 'gm'))) {
        expect(value, `mullvad-daemon ${key}`).toContain(PUBLISHER);
      }
    }
  });

  it('names the company as the organization of the iOS app', () => {
    const project = read('ios/WarrenVPN.xcodeproj/project.pbxproj');
    expect(fieldValues(project, /ORGANIZATIONNAME = "([^"]*)";/g)).toEqual([PUBLISHER]);
    const copyrights = fieldValues(project, /INFOPLIST_KEY_NSHumanReadableCopyright = "([^"]*)";/g);
    expect(copyrights).not.toHaveLength(0);
    for (const value of copyrights) {
      expect(value).toContain(PUBLISHER);
    }
  });
});
