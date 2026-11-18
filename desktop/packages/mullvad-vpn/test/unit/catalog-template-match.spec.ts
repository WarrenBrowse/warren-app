import fs from 'fs';
import { po } from 'gettext-parser';
import path from 'path';
import { describe, expect, it } from 'vitest';

// `update-translations` rewrites the template only, so a string removed from
// the code stayed in every catalog: they carried some 440 dead entries each
// before they were purged. The template is the list of what the app can show.
const LOCALES_DIR = path.resolve(import.meta.dirname, '../../locales');
const locales = fs
  .readdirSync(LOCALES_DIR)
  .filter((entry) => fs.statSync(path.join(LOCALES_DIR, entry)).isDirectory());

function entries(file: string): string[] {
  const parsed = po.parse(fs.readFileSync(file));
  return Object.entries(parsed.translations).flatMap(([context, messages]) =>
    Object.keys(messages)
      .filter((msgid) => msgid !== '')
      .map((msgid) => `${context}\u0004${msgid}`),
  );
}

const template = new Set(entries(path.join(LOCALES_DIR, 'messages.pot')));

describe.each(locales)('the %s catalog', (locale) => {
  it('carries no entry the template has dropped', () => {
    const obsolete = entries(path.join(LOCALES_DIR, locale, 'messages.po')).filter(
      (key) => !template.has(key),
    );
    expect(obsolete.map((key) => key.replace('\u0004', ' | '))).toEqual([]);
  });
});
