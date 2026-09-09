import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import { parseChangelog } from '../../src/main/changelog';

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
 */
const repoRoot = path.resolve(__dirname, '../../../../..');
const changesTxt = path.join(repoRoot, 'desktop/packages/mullvad-vpn/changes.txt');
const changelogMd = path.join(repoRoot, 'CHANGELOG.md');
const versionFile = path.join(repoRoot, 'dist-assets/desktop-product-version.txt');

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
function changelogSection(version: string): string {
  const lines = fs.readFileSync(changelogMd, 'utf8').split('\n');
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
    const blocks = parseChangelog(fs.readFileSync(changesTxt, 'utf8'));
    expect(blocks.length).toBeGreaterThan(0);
    expect(blocks.some((block) => block.type === 'list')).toBe(true);
  });

  it('is a section body, without the version heading the view already shows', () => {
    const bundled = fs.readFileSync(changesTxt, 'utf8');
    expect(bundled).not.toMatch(/^## \[/m);
  });
});
