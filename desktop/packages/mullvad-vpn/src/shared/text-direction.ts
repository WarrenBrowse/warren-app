export type TextDirection = 'ltr' | 'rtl';

// Languages written right to left. Only Arabic and Persian ship a catalog today; the others are
// listed so that a catalog added later is laid out correctly without touching this file.
const RIGHT_TO_LEFT_LANGUAGES = new Set([
  'ar',
  'fa',
  'he',
  'ur',
  'ps',
  'ckb',
  'sd',
  'ug',
  'yi',
  'dv',
]);

export function textDirection(locale: string): TextDirection {
  const language = locale.split(/[-_]/)[0].toLowerCase();
  return RIGHT_TO_LEFT_LANGUAGES.has(language) ? 'rtl' : 'ltr';
}
