import { expect, test } from '@playwright/test';
import { ConsoleMessage, Page } from 'playwright';

import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;
const violations: string[] = [];

// The page's Content-Security-Policy denies everything it does not use. These
// checks run the built renderer under it: the screens draw without a single
// refused load, and what the policy is there to refuse is refused.
test.describe('Content-Security-Policy of the renderer', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    page.on('console', (message: ConsoleMessage) => {
      if (message.text().includes('Content Security Policy')) {
        violations.push(message.text());
      }
    });
    await page.reload();
    await routes.main.waitForRoute();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('draws the main, settings and VPN settings screens without a refused load', async () => {
    await routes.main.gotoSettings();
    await routes.settings.gotoVpnSettings();
    await expect(page.getByText('Lockdown mode').first()).toBeVisible();

    expect(violations).toEqual([]);
  });

  test('refuses to compile WebAssembly', async () => {
    const outcome = await page.evaluate(async () => {
      try {
        await WebAssembly.compile(new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]));
        return 'compiled';
      } catch {
        return 'refused';
      }
    });

    expect(outcome).toBe('refused');
  });
});
