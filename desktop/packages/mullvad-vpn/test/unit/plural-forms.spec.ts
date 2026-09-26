import fs from 'fs';
import { po } from 'gettext-parser';
import Gettext from 'node-gettext';
import path from 'path';
import { describe, expect, it } from 'vitest';

// The renderer picks a plural form with node-gettext's own rule for the language, not with the
// Plural-Forms header of the catalog. For Persian that rule has a single form, so the first
// msgstr is shown for every count, and a first form written for "1" once made every duration
// read "1 day".
const LOCALES_DIR = path.resolve(import.meta.dirname, '../../locales');
const locales = fs
  .readdirSync(LOCALES_DIR)
  .filter((entry) => fs.statSync(path.join(LOCALES_DIR, entry)).isDirectory());

// Counts that no language gives a form of its own without the number (Arabic has its own words
// for zero, one and two, which may drop it).
const COUNTS = [3, 5, 11, 29, 100];
const PLACEHOLDER = /%(\([^)]+\))?d/;

describe.each(locales)('plural forms in %s', (locale) => {
  const parsed = po.parse(fs.readFileSync(path.join(LOCALES_DIR, locale, 'messages.po')));
  const catalogue = new Gettext();
  catalogue.addTranslations(locale, 'messages', parsed);
  catalogue.setLocale(locale);
  catalogue.setTextDomain('messages');

  const plurals = Object.entries(parsed.translations).flatMap(([context, entries]) =>
    Object.values(entries)
      .filter((entry) => entry.msgid_plural && PLACEHOLDER.test(entry.msgid_plural))
      .map((entry) => ({ context, entry })),
  );

  it('shows the count in the form the app picks for several items', () => {
    const missing = plurals.flatMap(({ context, entry }) =>
      COUNTS.filter(
        (count) =>
          !PLACEHOLDER.test(catalogue.npgettext(context, entry.msgid, entry.msgid_plural!, count)),
      ).map((count) => `${entry.msgid} (${count})`),
    );
    expect(missing).toEqual([]);
  });
});
