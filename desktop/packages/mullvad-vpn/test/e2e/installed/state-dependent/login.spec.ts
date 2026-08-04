import { expect, test } from '@playwright/test';
import { execSync } from 'child_process';
import { Page } from 'playwright';

import { RoutePath } from '../../../../src/shared/routes';
import { RoutesObjectModel } from '../../route-object-models';
import { TestUtils } from '../../utils';
import { startInstalledApp } from '../installed-utils';

// This test expects the daemon to be logged out.
// Env parameters:
//   `ACCOUNT_MNEMONIC`: a 12/24-word BIP39 recovery phrase whose
//   account is known to be expired, used to exercise the restore flow.
//
// Warren has no public-key login: an identity is a BIP39 recovery
// phrase. You either create a brand-new account (which mints a fresh
// phrase) or restore an existing one by entering its phrase.

let page: Page;
let util: TestUtils;
let routes: RoutesObjectModel;

test.beforeAll(async () => {
  ({ page, util } = await startInstalledApp());
  routes = new RoutesObjectModel(page, util);
});

test.afterAll(async () => {
  await util?.closePage();
});

test('App should show the welcome screen', async () => {
  await util.expectRoute(RoutePath.login);

  await expect(page.locator('h1')).toHaveText('Welcome to Warren');
  await expect(routes.login.selectors.createAccountButton()).toBeVisible();
  await expect(routes.login.selectors.restoreButton()).toBeVisible();
});

test('App should create a new account after backing up the phrase', async () => {
  await util.expectRoute(RoutePath.login);

  await routes.login.createNewAccount();
  await expect(page.locator('h1')).toHaveText('Back up your recovery phrase');
  await expect(routes.login.selectors.mnemonicGrid()).toBeVisible();

  // A brand-new account has no subscription yet → expired screen.
  await routes.login.confirmBackup();
  await util.expectRoute(RoutePath.expired);

  // Clean up the freshly created identity from the daemon.
  execSync('mullvad account logout');
});

test('App should become logged out', async () => {
  await util.expectRoute(RoutePath.login);
});

test('App should restore an expired account from its recovery phrase', async () => {
  test.skip(!process.env.ACCOUNT_MNEMONIC, 'ACCOUNT_MNEMONIC not provided');
  await util.expectRoute(RoutePath.login);

  await routes.login.startRestore();
  await expect(page.locator('h1')).toHaveText('Restore your account');

  await routes.login.fillRecoveryPhrase(process.env.ACCOUNT_MNEMONIC!);
  await routes.login.submitRestore();

  await util.expectRoute(RoutePath.expired);

  execSync('mullvad account logout');
});
