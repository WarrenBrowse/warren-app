import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

/**
 * The consent prompt of the forum sign-in flow, read straight off the
 * translation catalogs. Two properties are worth a gate rather than a review:
 *
 * The cross-device prompt is raised for two inputs that share nothing but
 * their uncertainty, a deep link carrying `xd=1` (the QR on the approval page)
 * and a code typed under Settings, which arrives with no link and no signal at
 * all. So its copy must not tell the reader where the request came from: a
 * typed code has no QR anywhere in the flow.
 *
 * And the prompt is the only place the relayed-approval attack is explained,
 * so a locale that falls back to English there is a locale where the warning
 * is not read. Half this flow shipped translated and half did not.
 */
const localesDir = path.resolve(__dirname, '../../locales');

/** `msgctxt "forum-login"` entries of a catalog, msgid to msgstr. */
function forumLoginEntries(file: string): Map<string, string> {
  const catalog = fs.readFileSync(file, 'utf8');
  const entry = /msgctxt "forum-login"\nmsgid "((?:[^"\\]|\\.)*)"\nmsgstr "((?:[^"\\]|\\.)*)"/g;
  const entries = new Map<string, string>();
  for (const match of catalog.matchAll(entry)) {
    entries.set(unescapePo(match[1]), unescapePo(match[2]));
  }
  return entries;
}

function unescapePo(value: string): string {
  return value.replace(/\\(.)/g, (_, char: string) => (char === 'n' ? '\n' : char));
}

function localeCatalogs(): [string, string][] {
  return fs
    .readdirSync(localesDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry): [string, string] => [
      entry.name,
      path.join(localesDir, entry.name, 'messages.po'),
    ])
    .filter(([, file]) => fs.existsSync(file))
    .sort(([a], [b]) => a.localeCompare(b));
}

const template = forumLoginEntries(path.join(localesDir, 'messages.pot'));

/**
 * "QR" survives untranslated in most locales; Simplified Chinese and Thai
 * spell it out, so both spellings are listed rather than assuming the Latin
 * token covers every language.
 */
const qrTokens = ['QR', '二维码', '二維碼', 'คิวอาร์'];

const crossDeviceIds = [
  'Sign in to the forum on another device?',
  'Warren cannot tell which browser is being signed in, or whether it is in front of you right now. Your app will sign a one-time challenge with your wallet key to prove it is you.',
  'Approve only if you are looking at that sign-in page right now. If someone sent you this code, they are signing in as you. No email and no password are used.',
];

describe('the forum sign-in consent prompt', () => {
  it('offers every cross-device string the prompt renders', () => {
    // A renamed source string would otherwise silence both gates below.
    for (const id of crossDeviceIds) {
      expect([...template.keys()], id).toContain(id);
    }
  });

  it('never names a QR code, the one origin a typed code cannot have', () => {
    const failures: string[] = [];
    const source: [string, Map<string, string>] = [
      'source',
      new Map(crossDeviceIds.map((id) => [id, id])),
    ];
    const translated: [string, Map<string, string>][] = localeCatalogs().map(([locale, file]) => [
      locale,
      forumLoginEntries(file),
    ]);
    for (const [locale, entries] of [source, ...translated]) {
      for (const id of crossDeviceIds) {
        const rendered = entries.get(id);
        if (rendered === undefined) {
          continue;
        }
        for (const token of qrTokens) {
          if (rendered.toUpperCase().includes(token.toUpperCase())) {
            failures.push(`${locale}: "${id.slice(0, 40)}..." names "${token}"`);
          }
        }
      }
    }
    expect(failures).toEqual([]);
  });

  it('is translated in every locale, so the warning is never read in English', () => {
    const failures: string[] = [];
    for (const [locale, file] of localeCatalogs()) {
      const entries = forumLoginEntries(file);
      for (const id of template.keys()) {
        if (!entries.get(id)) {
          failures.push(`${locale}: untranslated "${id.slice(0, 50)}..."`);
        }
      }
    }
    expect(failures).toEqual([]);
  });
});
