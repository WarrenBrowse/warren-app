import { describe, expect, it } from 'vitest';

import { UnsupportedVersionNotificationProvider } from '../../src/shared/notifications';
import { RoutePath } from '../../src/shared/routes';

function bannerAction(manualInstallOnly: boolean) {
  const provider = new UnsupportedVersionNotificationProvider({
    supported: false,
    consistent: true,
    suggestedIsBeta: false,
    suggestedUpgrade: { changelog: [], version: '2100.1', manualInstallOnly },
  });
  const subtitle = provider.getInAppNotification().subtitle;
  return Array.isArray(subtitle) ? subtitle[1]?.action : undefined;
}

// The forced-update banner leads to the in-app upgrade whenever the daemon can
// install one, Linux included, and to the website only when it cannot.
describe('unsupported version banner', () => {
  it('opens the in-app upgrade when an installer fits this install', () => {
    expect(bannerAction(false)).toEqual({
      type: 'navigate-internal',
      link: { to: RoutePath.appUpgrade },
    });
  });

  it('sends an install no installer fits to the download page', () => {
    expect(bannerAction(true)?.type).toBe('navigate-external');
  });
});
