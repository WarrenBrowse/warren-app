import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(__dirname, '../../../../..');

function read(relative: string): string {
  return fs.readFileSync(path.join(repoRoot, relative), 'utf8');
}

// The English source catalogue of each client: what the user reads before
// buying, in every language translated from it.
const CATALOGUES = [
  'desktop/packages/mullvad-vpn/locales/messages.pot',
  'android/lib/ui/resource/src/main/res/values/strings.xml',
  'ios/Assets/Onboarding.xcstrings',
];

// Text shown before a purchase has to describe the contract the user enters.
// Time is prepaid and may renew every month, which the account screen manages,
// so the onboarding cannot promise the opposite.
describe('the pricing promises of the onboarding', () => {
  for (const catalogue of CATALOGUES) {
    it(`never deny recurring billing in ${catalogue}`, () => {
      const text = read(catalogue).toLowerCase();

      expect(text).not.toContain('no recurring billing');
      expect(text).not.toContain('pay as you go');
    });
  }
});

// One name per concept in each language, the same on the three clients. The
// secret had three French names, "kill switch" was translated although French
// speakers say it in English, and "Aucune trace" promised what the content
// policy forbids promising.
const FRENCH_CATALOGUES = [
  'desktop/packages/mullvad-vpn/locales/fr/messages.po',
  'android/lib/ui/resource/src/main/res/values-fr/strings.xml',
  'ios/Assets/Localizable.xcstrings',
  'ios/Assets/Onboarding.xcstrings',
  'ios/Assets/Settings.xcstrings',
  'ios/Assets/Wallet.xcstrings',
];

describe('the French glossary', () => {
  for (const catalogue of FRENCH_CATALOGUES) {
    it(`keeps to one name per concept in ${catalogue}`, () => {
      const text = read(catalogue).toLowerCase();

      for (const offGlossary of [
        'phrase de restauration',
        'phrase mnémonique',
        'coupe-circuit',
        'aucune trace',
        'bienvenue sur warren',
      ]) {
        expect(text).not.toContain(offGlossary);
      }
    });
  }
});

// The hybrid post-quantum seal follows the exit: an exit that publishes an
// ML-KEM key gets it, whatever the settings say. A switch for it would change
// a setting the tunnel never reads, and turning it off would not turn the
// protection off.
describe('the post-quantum seal', () => {
  it('is not offered as a setting on the desktop', () => {
    const template = read('desktop/packages/mullvad-vpn/locales/messages.pot');

    expect(template).not.toContain('msgid "Quantum-resistant tunnel"');
  });
});
