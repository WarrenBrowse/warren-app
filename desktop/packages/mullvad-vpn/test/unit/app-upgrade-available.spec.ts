import { describe, expect, it, vi } from 'vitest';

import { AppUpgradeAvailableNotificationProvider } from '../../src/renderer/lib/notifications/app-upgrade-available';
import { RoutePath } from '../../src/shared/routes';

const provider = (platform: NodeJS.Platform, suggestedIsBeta = false) =>
  new AppUpgradeAvailableNotificationProvider({
    suggestedUpgradeVersion: '1.4.0',
    suggestedIsBeta,
    platform,
    close: vi.fn(),
  });

// The home's update card (mockup 8d): a download glyph, the version, and an
// Install button.
describe('the update available banner', () => {
  it('names the version under a short title and leads with the download glyph', () => {
    const notification = provider('darwin').getInAppNotification();
    expect(notification.icon).toBe('update');
    expect(notification.title).toBe('Update available');
    expect(notification.subtitle).toBe('Version 1.4.0');
  });

  it('installs through the upgrade view where the app upgrades itself', () => {
    const { button } = provider('darwin').getInAppNotification();
    expect(button?.label).toBe('Install');
    expect(button?.action).toMatchObject({
      type: 'navigate-internal',
      link: { to: RoutePath.appUpgrade },
    });
  });

  it('installs from the download page on Linux, where packages are installed by hand', () => {
    expect(provider('linux').getInAppNotification().button?.action.type).toBe('navigate-external');
  });

  it('lets a beta offer be put away', () => {
    expect(provider('darwin', true).getInAppNotification().action?.type).toBe('close');
    expect(provider('darwin', false).getInAppNotification().action).toBeUndefined();
  });
});
