import { describe, expect, it } from 'vitest';

import { countryCodeFromName } from '../../src/renderer/lib/country-code';

describe('countryCodeFromName', () => {
  it('resolves relay-list countries first (authoritative, any future exit)', () => {
    const relays = [{ name: 'Fantasialand', code: 'FL' }];
    expect(countryCodeFromName('Fantasialand', relays)).toBe('fl');
  });

  it('resolves world countries via CLDR display names without a relay entry', () => {
    expect(countryCodeFromName('Netherlands')).toBe('nl');
    expect(countryCodeFromName('Germany')).toBe('de');
    expect(countryCodeFromName('Singapore')).toBe('sg');
    expect(countryCodeFromName('France')).toBe('fr');
  });

  it('is case- and whitespace-insensitive', () => {
    expect(countryCodeFromName('  netherlands ')).toBe('nl');
  });

  it('resolves common geoip aliases that differ from CLDR', () => {
    expect(countryCodeFromName('Czech Republic')).toBe('cz');
    expect(countryCodeFromName('South Korea')).toBe('kr');
    expect(countryCodeFromName('Russia')).toBe('ru');
  });

  it('returns undefined for unknown or empty names', () => {
    expect(countryCodeFromName('Atlantis')).toBeUndefined();
    expect(countryCodeFromName('')).toBeUndefined();
    expect(countryCodeFromName(undefined)).toBeUndefined();
  });
});
