import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

// The Support view is where a user looks for the documents that govern the
// service: the terms and the privacy policy sit side by side, as on Android
// and iOS.
test.describe('Support view', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await routes.main.waitForRoute();
    await routes.main.gotoSettings();
    await page.getByRole('button', { name: 'Support' }).click();
    await util.expectRoute(RoutePath.support);
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('links the terms of use and the privacy policy', async () => {
    await expect(page.getByRole('button', { name: 'Terms of use' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Privacy policy' })).toBeVisible();
  });
});
