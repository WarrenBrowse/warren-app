import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import { parseChangelog } from '../../src/main/changelog';

// `parseChangelog` keeps only the entries for the platform it runs on, and CI
// runs these tests on Linux: a release whose notes are all macOS entries (1.1.34)
// parsed to nothing there although every Mac renders it. What the file must
// guarantee is a renderable list on at least one of the platforms it targets.
function rendersAListOnSomePlatform(text: string): boolean {
  const original = Object.getOwnPropertyDescriptor(process, 'platform')!;
  try {
    return ['darwin', 'linux', 'win32'].some((platform) => {
      Object.defineProperty(process, 'platform', { ...original, value: platform });
      return parseChangelog(text).some((block) => block.type === 'list');
    });
  } finally {
    Object.defineProperty(process, 'platform', original);
  }
}

/**
 * The bundled "What's new" screen must show THIS version's notes.
 *
 * `changes.txt` is packaged with the app (the `files` list in
 * `tasks/distribution.cjs`) and read by `readChangelog()`, which the view
 * renders under the running version number. Upstream maintains that file by
 * hand, and in this fork it kept the fork's own text, "First public beta
 * release.", from 1.1.0 to 1.1.28: 25 releases told every user who opened the
 * screen that nothing had changed since the first beta, while the release body
 * and the update prompt (both generated from `CHANGELOG.md`) were correct.
 *
 * Nothing compared the two, so nothing could notice. This suite is that
 * comparison, and it runs on the Node-only desktop runner: no cargo, no
 * Electron, no network.
 *
 * It covers every language the app ships notes in. The screen reads a file per
 * language (`readChangelog`), and a translated file left behind shows a French
 * reader the previous release's notes, which is the same defect as staleness in
 * English with one more way to go unnoticed.
 */
const repoRoot = path.resolve(__dirname, '../../../../..');
const changesTxt = path.join(repoRoot, 'desktop/packages/mullvad-vpn/changes.txt');
const changelogMd = path.join(repoRoot, 'CHANGELOG.md');
const versionFile = path.join(repoRoot, 'dist-assets/desktop-product-version.txt');

/**
 * The languages the bundled screen ships, keyed by the file suffix
 * `readChangelog` resolves from the app locale. Their source is the
 * `CHANGELOG.<lang>.md` sibling, which also feeds the signed update manifest
 * through `ci/build-version-metadata.py`, so both surfaces say the same thing.
 */
const translations = [
  { language: 'fr', changes: 'changes.fr.txt', changelog: 'CHANGELOG.fr.md' },
  { language: 'ro', changes: 'changes.ro.txt', changelog: 'CHANGELOG.ro.md' },
];

/**
 * The committed desktop version, which is what a build of this commit reports
 * and therefore the version the screen puts above these notes. A dev build
 * appends `-dev-<hash>`; the changelog is keyed on the release version.
 */
function committedVersion(): string {
  return fs.readFileSync(versionFile, 'utf8').trim().split('-')[0];
}

/**
 * Independent reader of `ci/extract-changelog-section.sh`. Deliberately not a
 * call into that script: a gate that reuses the generator's own code cannot
 * catch the generator being wrong, only the file being unwritten.
 */
function changelogSection(version: string, file: string = changelogMd): string {
  const lines = fs.readFileSync(file, 'utf8').split('\n');
  const captured: string[] = [];
  let capturing = false;
  for (const line of lines) {
    if (line.startsWith('## ')) {
      if (capturing) break;
      capturing = line.includes(`[${version}]`);
      continue;
    }
    if (capturing) captured.push(line);
  }
  return captured.join('\n').trim();
}

describe("the bundled what's-new screen", () => {
  const version = committedVersion();

  it('carries the notes of the committed version, not an older release', () => {
    const bundled = fs.readFileSync(changesTxt, 'utf8').trim();
    const expected = changelogSection(version);

    // Names the fix in the failure message: this suite is most likely to go
    // red on someone preparing a release, who should not have to read the spec.
    expect(
      expected,
      `CHANGELOG.md has no [${version}] section; write the release notes first`,
    ).not.toBe('');
    expect(
      bundled,
      `changes.txt is stale for ${version}; run scripts/release/generate-changes-txt.sh`,
    ).toBe(expected);
  });

  it('parses into renderable blocks rather than one opaque paragraph', () => {
    // A file the parser cannot structure renders as a wall of text under the
    // version heading, which is the same user-visible defect as staleness.
    expect(rendersAListOnSomePlatform(fs.readFileSync(changesTxt, 'utf8'))).toBe(true);
  });

  it('is a section body, without the version heading the view already shows', () => {
    const bundled = fs.readFileSync(changesTxt, 'utf8');
    expect(bundled).not.toMatch(/^## \[/m);
  });

  describe.each(translations)('in $language', ({ changes, changelog }) => {
    const bundledPath = path.join(repoRoot, 'desktop/packages/mullvad-vpn', changes);
    const sourcePath = path.join(repoRoot, changelog);

    it('is shipped at all, so the screen is not English for everyone', () => {
      expect(
        fs.existsSync(bundledPath),
        `${changes} is missing; run scripts/release/generate-changes-txt.sh`,
      ).toBe(true);
    });

    it('carries the notes of the committed version', () => {
      const bundled = fs.readFileSync(bundledPath, 'utf8').trim();
      const expected = changelogSection(version, sourcePath);

      expect(
        expected,
        `${changelog} has no [${version}] section; translate the release notes first`,
      ).not.toBe('');
      expect(
        bundled,
        `${changes} is stale for ${version}; run scripts/release/generate-changes-txt.sh`,
      ).toBe(expected);
    });

    it('parses into renderable blocks rather than one opaque paragraph', () => {
      expect(rendersAListOnSomePlatform(fs.readFileSync(bundledPath, 'utf8'))).toBe(true);
    });
  });
});
