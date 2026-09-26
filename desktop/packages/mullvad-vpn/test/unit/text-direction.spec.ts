import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import { applyDocumentLocale } from '../../src/renderer/lib/document-locale';
import { textDirection } from '../../src/shared/text-direction';

const LOCALES_DIR = path.resolve(import.meta.dirname, '../../locales');
const shippedLocales = fs
  .readdirSync(LOCALES_DIR)
  .filter((entry) => fs.statSync(path.join(LOCALES_DIR, entry)).isDirectory());

describe('textDirection', () => {
  it('reads Arabic and Persian right to left, whatever the region', () => {
    expect(textDirection('ar')).toBe('rtl');
    expect(textDirection('fa')).toBe('rtl');
    expect(textDirection('ar-EG')).toBe('rtl');
    expect(textDirection('fa_IR')).toBe('rtl');
  });

  it('reads every other shipped catalog left to right', () => {
    const ltr = shippedLocales.filter((locale) => !['ar', 'fa'].includes(locale));
    expect(ltr.length).toBeGreaterThan(15);
    for (const locale of ltr) {
      expect(textDirection(locale), locale).toBe('ltr');
    }
  });

  it('reads English and the development pseudo-locale left to right', () => {
    expect(textDirection('en')).toBe('ltr');
    expect(textDirection('en-US')).toBe('ltr');
    expect(textDirection('sv-rö')).toBe('ltr');
  });
});

describe('applyDocumentLocale', () => {
  const root = () => ({ lang: '', dir: '' });

  it('marks the document with the catalog language and its direction', () => {
    const element = root();
    applyDocumentLocale(element, 'fa', true);
    expect(element).toEqual({ lang: 'fa', dir: 'rtl' });
  });

  it('follows a language change back to left to right', () => {
    const element = root();
    applyDocumentLocale(element, 'ar', true);
    applyDocumentLocale(element, 'de', true);
    expect(element).toEqual({ lang: 'de', dir: 'ltr' });
  });

  // A right-to-left system language without a catalog shows the English source strings, which
  // must not be laid out right to left.
  it('falls back to English when no catalog was loaded for the locale', () => {
    const element = root();
    applyDocumentLocale(element, 'he', false);
    expect(element).toEqual({ lang: 'en', dir: 'ltr' });
  });
});
