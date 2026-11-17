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
