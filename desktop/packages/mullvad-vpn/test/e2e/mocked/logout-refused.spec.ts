import { expect, test } from '@playwright/test';
import fs from 'fs';
import { po } from 'gettext-parser';
import path from 'path';
import { Page } from 'playwright';

import { RoutePath } from '../../../src/shared/routes';
import { textDirection } from '../../../src/shared/text-direction';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

const LOCALES_DIR = path.resolve(import.meta.dirname, '../../../locales');
const SCREENSHOTS = process.env.LOGOUT_REFUSED_SCREENSHOTS ?? 'test-results/logout-refused';

function catalog(locale: string) {
  const parsed = po.parse(fs.readFileSync(path.join(LOCALES_DIR, locale, 'messages.po')));
  return (context: string, msgid: string) => {
    const msgstr = parsed.translations[context]?.[msgid]?.msgstr[0];
    expect(msgstr, `${locale} translates "${msgid}"`).toBeTruthy();
    return msgstr!;
  };
}

async function start(locale: string) {
  process.env.WARREN_E2E_LOCALE = locale;
  ({ page, util } = await startMockedApp());
  delete process.env.WARREN_E2E_LOCALE;
  routes = new RoutesObjectModel(page, util);
  await util.ipc.guiSettings[''].notify({
    preferredLocale: locale,
    enableSystemNotifications: true,
    autoConnect: false,
    monochromaticIcon: false,
    startMinimized: false,
    unpinnedWindow: true,
    browsedForSplitTunnelingApplications: [],
    changelogDisplayedForVersion: '',
    updateDismissedForVersion: '',
    animateMap: false,
    onboardingPending: false,
  });
  await routes.main.waitForRoute();
}

// Logs out from the account view, confirming the backup, with the daemon
// answering `outcome`.
async function logOutAnswered(t: ReturnType<typeof catalog>, outcome: 'tunnel-still-up') {
  await page.getByRole('button', { name: t('', 'Account settings') }).click();
  await util.expectRoute(RoutePath.account);
  await page.getByRole('button', { name: t('account-view', 'Log out') }).click();
  await page
    .getByText(
      t(
        'account-view',
        'I have backed up my recovery phrase and understand this account will be removed from this device.',
      ),
    )
    .click();
  await Promise.all([
    util.ipc.account.logout.expect(outcome),
    page
      .getByRole('dialog')
      .getByRole('button', { name: t('account-view', 'Log out') })
      .click(),
  ]);
}

// The daemon refuses a logout whose tunnel did not come down within its bound,
// and changes nothing. The view the user logged out from says so, and its
// retry sends the logout again without asking for the backup confirmation.
test.describe('Logout refused', () => {
  test.beforeAll(async () => {
    await start('en');
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('says why the account is still logged in and retries', async () => {
    await routes.main.gotoAccount();

    await page.getByRole('button', { name: 'Log out' }).click();
    await page.getByText('I have backed up my recovery phrase').click();
    await Promise.all([
      util.ipc.account.logout.expect('tunnel-still-up'),
      page.getByRole('dialog').getByRole('button', { name: 'Log out' }).click(),
    ]);

    const failure = page.getByTestId('logout-failure');
    await expect(failure.getByRole('alert')).toHaveText(
      'Could not log out: the VPN did not disconnect in time. You are still logged in.',
    );
    await util.expectRoute(RoutePath.account);

    const retried = util.ipc.account.logout.expect('logged-out');
    await failure.getByRole('button', { name: 'Try again' }).click();
    await expect(retried).resolves.toBe('gui-logout-button');
    await expect(failure).toHaveCount(0);
  });
});

// Arabic and Persian read right to left: the failure and its retry are laid
// out that way, in the catalog's own words. Screenshots land in
// <LOGOUT_REFUSED_SCREENSHOTS>/<locale>.png.
for (const locale of (process.env.LOGOUT_REFUSED_LOCALES ?? 'ar,fa').split(',')) {
  test.describe(`Logout refused (${locale})`, () => {
    const t = catalog(locale);

    test.beforeAll(async () => {
      await start(locale);
    });

    test.afterAll(async () => {
      await util?.closePage();
    });

    test('says why in the catalog, right to left', async () => {
      await expect(page.locator('html')).toHaveAttribute('dir', textDirection(locale));

      await logOutAnswered(t, 'tunnel-still-up');

      const failure = page.getByTestId('logout-failure');
      await expect(failure.getByRole('alert')).toHaveText(
        t(
          'account-view',
          'Could not log out: the VPN did not disconnect in time. You are still logged in.',
        ),
      );
      await expect(
        failure.getByRole('button', { name: t('account-view', 'Try again') }),
      ).toBeVisible();
      await page.screenshot({ path: `${SCREENSHOTS}/${locale}.png` });
    });
  });
}
