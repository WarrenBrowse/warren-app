import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import {
  AuthFailedError,
  ErrorStateCause,
  TunnelState,
} from '../../../src/shared/daemon-rpc-types';
import { DeviceListResult, IDevice } from '../../../src/shared/devices';
import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { NavigationObjectModel } from '../route-object-models/navigation';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

// Screenshots of the list, the confirmation and both banners land here.
const SCREENSHOTS = process.env.MANAGE_DEVICES_SCREENSHOTS ?? 'test-results/manage-devices';

const today = new Date().toISOString().slice(0, 10);

const devices: IDevice[] = [
  {
    id: 'AAAAAAAAAAAAAAAAAAAAAA',
    name: 'Old owl',
    platform: 'windows',
    appVersion: '1.1.40',
    registeredOn: '2026-08-01',
    lastSeenOn: '2026-09-14',
    thisDevice: false,
  },
  {
    id: 'BBBBBBBBBBBBBBBBBBBBBB',
    name: 'Blue fox',
    platform: 'macos',
    appVersion: '1.1.56',
    registeredOn: '2026-10-01',
    lastSeenOn: today,
    thisDevice: true,
  },
  {
    id: 'CCCCCCCCCCCCCCCCCCCCCC',
    name: 'Red cat',
    platform: 'android',
    appVersion: '1.1.55',
    registeredOn: '2026-10-02',
    lastSeenOn: '2026-10-27',
    thisDevice: false,
  },
];

const listed: DeviceListResult = {
  type: 'listed',
  list: { devices, olderClientSeenOn: '2026-10-25' },
};

function blockedFor(authFailedError: AuthFailedError): TunnelState {
  return {
    state: 'error',
    details: { cause: ErrorStateCause.authFailed, authFailedError },
  } as TunnelState;
}

test.describe('Manage devices view', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await routes.main.waitForRoute();
    await util.ipc.devices.list.handle(listed);
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('opens from Account with the count of active devices', async () => {
    await routes.main.gotoAccount();
    const row = page.getByTestId('devices-row');
    await expect(row).toContainText('3');

    await row.click();
    await util.expectRoute(RoutePath.devices);
  });

  test('lists this device first, then by last activity, and the older client', async () => {
    const rows = page.getByTestId('device-row');
    await expect(rows).toHaveCount(3);
    await expect(rows.nth(0)).toContainText('Blue fox');
    await expect(rows.nth(0)).toContainText('This device');
    await expect(rows.nth(0)).toContainText('Active today');
    await expect(rows.nth(0).getByRole('button', { name: 'Log out' })).toBeVisible();
    await expect(rows.nth(1)).toContainText('Red cat');
    await expect(rows.nth(2)).toContainText('Old owl');
    await expect(page.getByTestId('older-client-row')).toContainText('An older version of Warren');
    await page.screenshot({ path: `${SCREENSHOTS}/list.png` });
  });

  test('confirms before disconnecting another device, then revokes it', async () => {
    const row = page.getByTestId('device-row').filter({ hasText: 'Red cat' });
    await row.getByRole('button', { name: 'Disconnect' }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toContainText('Disconnect Red cat?');
    await expect(dialog).toContainText('It will be signed out within 10 minutes');
    await page.screenshot({ path: `${SCREENSHOTS}/confirm-disconnect.png` });

    const removed = util.ipc.devices.remove.expect({ type: 'done' });
    await dialog.getByRole('button', { name: 'Disconnect' }).click();
    await expect(removed).resolves.toBe('CCCCCCCCCCCCCCCCCCCCCC');
  });

  test('renames a device to the name typed', async () => {
    const row = page.getByTestId('device-row').filter({ hasText: 'Old owl' });
    await row.getByRole('button', { name: 'Rename device' }).click();

    const dialog = page.getByRole('dialog');
    await dialog.getByRole('textbox').fill('Kitchen laptop');
    await page.screenshot({ path: `${SCREENSHOTS}/rename.png` });
    const renamed = util.ipc.devices.rename.expect({ type: 'done' });
    await dialog.getByRole('button', { name: 'Save' }).click();
    await expect(renamed).resolves.toEqual({
      id: 'AAAAAAAAAAAAAAAAAAAAAA',
      name: 'Kitchen laptop',
    });
  });
});

test.describe('Blocked-state banners of a device problem', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await routes.main.waitForRoute();
    await util.ipc.devices.list.handle(listed);
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  for (const [name, reason, text] of [
    [
      'no-session-token',
      AuthFailedError.noSessionToken,
      'This device has no session credentials right now.',
    ],
    [
      'device-limit',
      AuthFailedError.tooManyConnections,
      'Too many simultaneous connections on this account.',
    ],
  ] as const) {
    test(`offers the device list (${name})`, async () => {
      await util.ipc.tunnel[''].notify(blockedFor(reason));
      await expect(page.getByText(text)).toBeVisible();
      await page.screenshot({ path: `${SCREENSHOTS}/banner-${name}.png` });

      await page.getByRole('link', { name: 'Manage devices' }).click();
      await util.expectRoute(RoutePath.devices);
      await page.getByTestId('device-row').first().waitFor();
      await new NavigationObjectModel(page, util).goBackToRoute(RoutePath.main);
    });
  }
});
