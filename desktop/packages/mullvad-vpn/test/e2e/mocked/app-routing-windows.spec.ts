import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

const SCREENSHOTS = process.env.APP_ROUTING_SCREENSHOTS ?? 'test-results/app-routing';

let page: Page;
let util: MockedTestUtils;

test.describe('App routing on Windows', () => {
  test.beforeAll(async () => {
    process.env.WARREN_E2E_PLATFORM = 'win32';
    ({ page, util } = await startMockedApp());
    delete process.env.WARREN_E2E_PLATFORM;
    const routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);
    await util.ipc.splitTunneling.getApplications.handle({ fromCache: false, applications: [] });
    await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications: [] });

    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('offers VPN only for like the other tabs', async () => {
    const includeOnly = page.getByRole('tab', { name: 'VPN only for' });

    await expect(includeOnly).toBeEnabled();
    await expect(page.getByTestId('include-only-coming-soon')).toHaveCount(0);
    await page.getByRole('tab', { name: 'Bypass VPN' }).focus();
    await page.keyboard.press('End');
    await expect(includeOnly).toBeFocused();
    await includeOnly.click();
    await expect(includeOnly).toHaveAttribute('aria-selected', 'true');
    await page.screenshot({ path: `${SCREENSHOTS}/16-windows-include-only.png` });
  });
});
