// Resolves an English country NAME (the only thing the daemon's geoip and the
// relay list expose) to an ISO 3166-1 alpha-2 code, which keys the bundled flag
// set (assets/images/flags/<code>.svg, plus xx.svg as the unknown fallback).

// Geoip and relay-list names that differ from the CLDR English display names.
const NAME_ALIASES: Readonly<Record<string, string>> = {
  'czech republic': 'cz',
  'south korea': 'kr',
  'north korea': 'kp',
  russia: 'ru',
  moldova: 'md',
  bolivia: 'bo',
  venezuela: 've',
  tanzania: 'tz',
  syria: 'sy',
  iran: 'ir',
  laos: 'la',
  brunei: 'bn',
  vietnam: 'vn',
  'macedonia (fyrom)': 'mk',
  macedonia: 'mk',
  'cape verde': 'cv',
  'ivory coast': 'ci',
  "cote d'ivoire": 'ci',
  'democratic republic of the congo': 'cd',
  'republic of the congo': 'cg',
  micronesia: 'fm',
  palestine: 'ps',
  'vatican city': 'va',
  'east timor': 'tl',
  swaziland: 'sz',
  burma: 'mm',
  usa: 'us',
  uk: 'gb',
};

// Deprecated / transitionally-reserved ISO codes whose CLDR display name
// collides with the canonical country (DD "Germany", FX "France", SU
// "Russia"...). Never let them claim a name in the reverse map.
const DEPRECATED_CODES = new Set([
  'an',
  'bu',
  'cs',
  'dd',
  'fx',
  'nt',
  'su',
  'tp',
  'yd',
  'yu',
  'zr',
]);

// CLDR English name -> code for every ISO alpha-2 code, built once. Codes whose
// display name comes back unchanged are unassigned and skipped.
let reverseCldr: Map<string, string> | undefined;

function cldrMap(): Map<string, string> {
  if (reverseCldr) {
    return reverseCldr;
  }
  reverseCldr = new Map();
  const displayNames = new Intl.DisplayNames('en', { type: 'region' });
  const a = 'a'.charCodeAt(0);
  for (let i = 0; i < 26; i++) {
    for (let j = 0; j < 26; j++) {
      const code = String.fromCharCode(a + i) + String.fromCharCode(a + j);
      if (DEPRECATED_CODES.has(code)) {
        continue;
      }
      try {
        const name = displayNames.of(code.toUpperCase());
        if (name && name.toLowerCase() !== code && !reverseCldr.has(name.toLowerCase())) {
          reverseCldr.set(name.toLowerCase(), code);
        }
      } catch {
        // Invalid region code: skip.
      }
    }
  }
  return reverseCldr;
}

export interface CountryWithCode {
  name: string;
  code: string;
}

/**
 * Country name -> ISO alpha-2 code. `relayCountries` (name + code straight from
 * the relay list) is authoritative and keeps every future Warren exit country
 * working with no app change; CLDR + aliases cover the local geoip country when
 * disconnected, which can be anywhere in the world.
 */
export function countryCodeFromName(
  name: string | undefined,
  relayCountries: readonly CountryWithCode[] = [],
): string | undefined {
  const key = (name ?? '').trim().toLowerCase();
  if (!key) {
    return undefined;
  }
  const fromRelays = relayCountries.find((country) => country.name.toLowerCase() === key)?.code;
  const code = fromRelays ?? NAME_ALIASES[key] ?? cldrMap().get(key);
  return code?.toLowerCase();
}
