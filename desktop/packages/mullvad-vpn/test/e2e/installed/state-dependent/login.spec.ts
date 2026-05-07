import { expect, test } from '@playwright/test';
import { exec, execSync } from 'child_process';
import { Page } from 'playwright';

import { RoutePath } from '../../../../src/shared/routes';
import { RoutesObjectModel } from '../../route-object-models';
import { expectDisconnected } from '../../shared/tunnel-state';
import { TestUtils } from '../../utils';
import { startInstalledApp } from '../installed-utils';

// This test expects the daemon to be logged out and the public key history to be cleared.
// Env parameters:
//   `ACCOUNT_NUMBER`: Warren pubkey (64-char hex) to use when logging in

let page: Page;
let util: TestUtils;
let routes: RoutesObjectModel;

let pubkey: string;

const INVALID_PUBKEY = '1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234ZZZZ';

test.beforeAll(async () => {
  ({ page, util } = await startInstalledApp());
  routes = new RoutesObjectModel(page, util);
});

test.afterAll(async () => {
  await util?.closePage();
});

test('App should fail to login', async () => {
  await util.expectRoute(RoutePath.login);

  const title = page.locator('h1');
  const subtitle = page.getByTestId('subtitle');

  await expect(title).toHaveText('Login');
  await expect(subtitle).toHaveText('Enter your public key');

  await routes.login.fillPubKey(INVALID_PUBKEY);
  await routes.login.loginByPressingEnter();

  await expect(title).toHaveText('Login failed');
  await expect(subtitle).toHaveText('Invalid public key');

  await routes.login.fillPubKey('');
});

test('App should create account', async () => {
  await util.expectRoute(RoutePath.login);

  await routes.login.createNewAccount();
  await util.expectRoute(RoutePath.expired);

  const outOfTimeTitle = page.getByTestId('title');
  await expect(outOfTimeTitle).toHaveText('Congrats!');

  const inputValue = await page.getByTestId('warren-pubkey').textContent();
  // 64 hex chars + 7 spaces (8 groups of 8) = 71 visible chars
  expect(inputValue).toHaveLength(71);
  pubkey = inputValue!.replaceAll(' ', '');
});

test('App should become logged out', async () => {
  exec('mullvad account logout');
  await util.expectRoute(RoutePath.login);
});

test('App should log in', async () => {
  await util.expectRoute(RoutePath.login);

  const title = page.locator('h1');
  const subtitle = page.getByTestId('subtitle');

  await expect(title).toHaveText('Login');
  await expect(subtitle).toHaveText('Enter your public key');

  await routes.login.fillPubKey(process.env.ACCOUNT_NUMBER!);
  await routes.login.loginByClickingLoginButton();

  await expect(title).toHaveText('Logged in');
  await expect(subtitle).toHaveText('Valid public key');

  await util.expectRoute(RoutePath.main);

  await expectDisconnected(page);
});

test('App should log out', async () => {
  await page.getByTestId('account-button').click();

  await util.expectRoute(RoutePath.account);

  await page.getByText('Log out').click();
  await util.expectRoute(RoutePath.login);

  const title = page.locator('h1');
  const subtitle = page.getByTestId('subtitle');
  await expect(title).toHaveText('Login');
  await expect(subtitle).toHaveText('Enter your public key');
});

test('App should log in to expired account', async () => {
  await util.expectRoute(RoutePath.login);

  const title = page.locator('h1');
  const subtitle = page.getByTestId('subtitle');

  await expect(title).toHaveText('Login');
  await expect(subtitle).toHaveText('Enter your public key');

  await routes.login.fillPubKey(pubkey);

  await routes.login.loginByPressingEnter();
  await util.expectRoute(RoutePath.expired);

  const outOfTimeTitle = page.getByTestId('title');
  await expect(outOfTimeTitle).toHaveText('Out of time');

  execSync('mullvad account logout');
});
