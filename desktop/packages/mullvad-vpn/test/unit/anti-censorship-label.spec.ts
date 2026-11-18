import { describe, expect, it } from 'vitest';

import { getAntiCensorshipLabel } from '../../src/renderer/components/views/vpn-settings/components/anti-censorship-list-item/utils';

// The row used to show the inherited obfuscation setting ("Automatic" by
// default), which the Warren tunnel never reads: its obfuscation is always on
// and the daemon reports whether it is.
describe('the anti-censorship row', () => {
  it('says the obfuscation is on when the daemon reports it active', () => {
    expect(getAntiCensorshipLabel(true)).toBe('On');
  });

  it('says it is off when the daemon reports it inactive', () => {
    expect(getAntiCensorshipLabel(false)).toBe('Off');
  });

  it('says it is on before the daemon has reported anything, like the view behind it', () => {
    expect(getAntiCensorshipLabel(undefined)).toBe('On');
  });
});
