import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

const START_DATE = new Date('2025-01-01T13:37:00');

const NON_EXPIRED_EXPIRY = {
  expiry: new Date(START_DATE.getTime() + 60 * 60 * 1000).toISOString(),
};

// A freshly minted Warren identity is an SS58 pubkey; the exact value
// is opaque to the GUI, which just echoes it back on finalize-login.
const NEW_PUBKEY = 'wb1234123412341234123412341234123412341234123412';
const RECOVERY_PHRASE =
  'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

test.describe('Login view', () => {
  const startup = async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
  };

  const logout = async () => {
    await util.ipc.account.device.notify({
      type: 'logged out',
      deviceState: { type: 'logged out' },
    });

    await routes.login.waitForRoute();
  };

  test.beforeAll(async () => {
    await startup();
    // The app boots a logged-in account into the onboarding wizard while
    // `onboardingCompletedUnix` is unset, so declare it complete: these tests
    // are about the logged-out login view, not about first-launch routing.
    await util.ipc.guiSettings[''].notify({
      preferredLocale: 'system',
      enableSystemNotifications: true,
      autoConnect: false,
      monochromaticIcon: false,
      startMinimized: false,
      unpinnedWindow: true,
      browsedForSplitTunnelingApplications: [],
      changelogDisplayedForVersion: '',
      updateDismissedForVersion: '',
      animateMap: false,
      onboardingCompletedUnix: Math.floor(START_DATE.getTime() / 1000),
    });
    await routes.main.waitForRoute();
  });

  test.beforeEach(async () => {
    await page.clock.install({ time: START_DATE });
    await logout();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('Should offer create and restore paths (no public-key input)', async () => {
    await expect(routes.login.selectors.header()).toHaveText('Welcome to Warren');
    await expect(routes.login.selectors.createAccountButton()).toBeVisible();
    await expect(routes.login.selectors.restoreButton()).toBeVisible();
  });

  test('Should require backing up the recovery phrase before finalizing a new account', async () => {
    await util.ipc.account.create.handle(NEW_PUBKEY);
    await util.ipc.account.getWarrenMnemonic.handle(RECOVERY_PHRASE);

    await routes.login.createNewAccount();

    // The daemon mints + logs into the new identity; the GUI holds on
    // the login screen to run the mandatory backup step.
    await util.ipc.account.device.notify({
      type: 'logged in',
      deviceState: {
        type: 'logged in',
        warrenIdentity: { pubkey: NEW_PUBKEY },
      },
    });

    // The minted phrase is shown for backup.
    await expect(routes.login.selectors.header()).toHaveText('Back up your recovery phrase');
    await expect(routes.login.selectors.mnemonicGrid()).toBeVisible();

    // Proceeding is gated behind the explicit "I saved it" confirmation.
    await expect(routes.login.selectors.backupContinueButton()).toBeDisabled();

    // After confirming, the brand-new (no-subscription) account routes
    // to the expired / buy-plan screen.
    await routes.login.confirmBackup();
    await page.clock.fastForward(1000);
    await routes.expired.waitForRoute();
  });

  // A user who taps "Create a new account" when they meant to restore one must
  // be able to get back to that choice. The identity is already minted by then,
  // so leaving discards it: the daemon logs out and the view returns to the
  // pick step, where "I already have an account" is reachable again.
  test('Should leave the backup step by discarding the new account', async () => {
    await util.ipc.account.create.handle(NEW_PUBKEY);
    await util.ipc.account.getWarrenMnemonic.handle(RECOVERY_PHRASE);

    await routes.login.createNewAccount();
    await util.ipc.account.device.notify({
      type: 'logged in',
      deviceState: {
        type: 'logged in',
        warrenIdentity: { pubkey: NEW_PUBKEY },
      },
    });
    await expect(routes.login.selectors.header()).toHaveText('Back up your recovery phrase');

    await Promise.all([util.ipc.account.logout.expect(), routes.login.discardNewAccount()]);

    await util.ipc.account.device.notify({
      type: 'logged out',
      deviceState: { type: 'logged out' },
    });

    await expect(routes.login.selectors.header()).toHaveText('Welcome to Warren');
    await expect(routes.login.selectors.restoreButton()).toBeVisible();
  });

  test('Should leave the restore step for the first screen', async () => {
    await routes.login.startRestore();
    await expect(routes.login.selectors.header()).toHaveText('Restore your account');

    await routes.login.leaveRestore();

    await expect(routes.login.selectors.header()).toHaveText('Welcome to Warren');
  });

  test('Should restore an account from a recovery phrase', async () => {
    await routes.login.startRestore();
    await expect(routes.login.selectors.header()).toHaveText('Restore your account');

    await routes.login.fillRecoveryPhrase(RECOVERY_PHRASE);

    await Promise.all([util.ipc.account.setWarrenMnemonic.expect(), routes.login.submitRestore()]);

    await util.ipc.account.device.notify({
      type: 'logged in',
      deviceState: {
        type: 'logged in',
        warrenIdentity: { pubkey: NEW_PUBKEY },
      },
    });
    await util.ipc.account[''].notify(NON_EXPIRED_EXPIRY);

    await page.clock.fastForward(1000);
    await routes.main.waitForRoute();
  });
});
