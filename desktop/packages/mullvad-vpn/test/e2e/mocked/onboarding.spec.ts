import { test } from '@playwright/test';
import { Page } from 'playwright';

import { IGuiSettingsState } from '../../../src/shared/gui-settings-state';
import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

// The main process broadcasts a navigation reset two minutes after the window
// is hidden, and again on system suspend. On macOS the window is attached to
// the tray and hides on every blur, so a logged-in user meets this several
// times a day.
const notifyNavigationReset = () => util.ipc.navigation.reset.notify();

const guiSettings = (onboardingPending: boolean): IGuiSettingsState => ({
  preferredLocale: 'en',
  autoConnect: false,
  enableSystemNotifications: true,
  monochromaticIcon: false,
  startMinimized: false,
  unpinnedWindow: false,
  browsedForSplitTunnelingApplications: [],
  changelogDisplayedForVersion: '',
  updateDismissedForVersion: '',
  animateMap: false,
  onboardingPending,
});

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

test.describe('Onboarding wizard', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);

    await util.ipc.guiSettings[''].notify(guiSettings(true));
    await util.expectRoute(RoutePath.onboardingWelcome);
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('Should return to its first step on a navigation reset while it is still owed', async () => {
    await page.getByTestId('onboarding-welcome-next').click();
    await util.expectRoute(RoutePath.onboardingWallet);

    await notifyNavigationReset();

    await util.expectRoute(RoutePath.onboardingWelcome);
  });

  // The wizard is a boot destination, so it sits at the bottom of the
  // navigation stack. Skipping it used to push the main view on top, which
  // left the wizard as the stack root, and every navigation reset popped the
  // user right back into it: the wizard reappeared after every skip, forever.
  test('Should not come back on a navigation reset once it has been skipped', async () => {
    await page.getByTestId('onboarding-skip').click();
    await util.expectRoute(RoutePath.main);

    await routes.main.gotoSettings();

    await notifyNavigationReset();

    await util.expectRoute(RoutePath.main);
  });
});
