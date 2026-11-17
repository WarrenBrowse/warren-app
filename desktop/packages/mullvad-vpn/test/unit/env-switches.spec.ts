import { describe, expect, it } from 'vitest';

import { devToolsPath, updateNotificationDisabled } from '../../src/main/env-switches';

// The environment switches carry the product's name. The upstream names are
// still read for one release, so an existing wrapper (the NixOS package) keeps
// working while it moves over.
describe('the GUI environment switches', () => {
  it('turn the update notification off under the Warren name', () => {
    expect(updateNotificationDisabled({ WARREN_DISABLE_UPDATE_NOTIFICATION: '1' })).toBe(true);
  });

  it('still honour the upstream name of the update switch', () => {
    expect(updateNotificationDisabled({ MULLVAD_DISABLE_UPDATE_NOTIFICATION: '1' })).toBe(true);
  });

  it('leave the update notification on otherwise', () => {
    expect(updateNotificationDisabled({})).toBe(false);
    expect(updateNotificationDisabled({ WARREN_DISABLE_UPDATE_NOTIFICATION: '0' })).toBe(false);
  });

  it('take the development tools path from the Warren name first', () => {
    expect(devToolsPath({ WARREN_PATH: '/w', MULLVAD_PATH: '/m' })).toBe('/w');
    expect(devToolsPath({ MULLVAD_PATH: '/m' })).toBe('/m');
    expect(devToolsPath({})).toBeUndefined();
  });
});
