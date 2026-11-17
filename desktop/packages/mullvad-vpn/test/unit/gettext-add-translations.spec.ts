import Gettext from 'node-gettext';
import { afterEach, describe, expect, it } from 'vitest';

import { addTranslations } from '../../src/shared/gettext';

const translations = {
  charset: 'utf-8',
  headers: {},
  translations: { '': { Connect: { msgid: 'Connect', msgstr: ['Conectare'] } } },
};

// node-gettext stores a catalogue at catalogs[locale][domain], so a `__proto__`
// locale writes the domain name onto Object.prototype.
function pollutedKeys(): string[] {
  return ['messages', 'charset', 'headers', 'translations'].filter((key) => key in {});
}

describe('addTranslations', () => {
  afterEach(() => {
    for (const key of pollutedKeys()) {
      delete (Object.prototype as Record<string, unknown>)[key];
    }
  });

  it('loads a catalogue for an ordinary locale', () => {
    const catalogue = new Gettext();

    expect(addTranslations(catalogue, 'ro', 'messages', translations)).toBe(true);
    catalogue.setTextDomain('messages');
    catalogue.setLocale('ro');

    expect(catalogue.gettext('Connect')).toBe('Conectare');
  });

  it.each(['__proto__', 'constructor', 'prototype'])(
    'refuses %s as a locale without touching the object prototype',
    (locale) => {
      const catalogue = new Gettext();

      const added = addTranslations(catalogue, locale, 'messages', translations);

      expect(pollutedKeys()).toEqual([]);
      expect(added).toBe(false);
    },
  );

  it.each(['__proto__', 'constructor', 'prototype'])('refuses %s as a domain', (domain) => {
    const catalogue = new Gettext();

    expect(addTranslations(catalogue, 'ro', domain, translations)).toBe(false);
  });
});
