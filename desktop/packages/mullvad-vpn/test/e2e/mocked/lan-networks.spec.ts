import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { ILanNetworks } from '../../../src/shared/daemon-rpc-types';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

const DEFAULT_NETWORKS = [
  '10.0.0.0/8',
  '172.16.0.0/12',
  '192.168.0.0/16',
  '169.254.0.0/16',
  'fe80::/10',
  'fc00::/7',
];

const notifyLanSettings = async (allowLan: boolean, lanNetworks: ILanNetworks) => {
  await util.ipc.settings[''].notify({ ...getDefaultSettings(), allowLan, lanNetworks });
};

test.describe('Local network sharing networks', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await routes.main.waitForRoute();
    await routes.main.gotoSettings();
    await routes.settings.gotoVpnSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('lists the shared networks only while sharing is on', async () => {
    await notifyLanSettings(false, { networks: DEFAULT_NETWORKS, custom: false });
    await expect(page.getByText('Add a network')).not.toBeVisible();

    await notifyLanSettings(true, { networks: DEFAULT_NETWORKS, custom: false });
    await expect(page.getByText('192.168.0.0/16')).toBeVisible();
    await expect(page.getByText('Reset to default')).not.toBeVisible();
  });

  test('adds a network to the current list', async () => {
    await notifyLanSettings(true, { networks: DEFAULT_NETWORKS, custom: false });
    await page.getByText('Add a network').click();
    await page.getByPlaceholder('Enter a network, e.g. 192.168.1.0/24').fill('400::/7');

    const [sent] = await Promise.all([
      util.ipc.settings.setLanNetworks.expect(),
      page.keyboard.press('Enter'),
    ]);
    expect(sent).toEqual([...DEFAULT_NETWORKS, '400::/7']);
  });

  test('refuses a network too broad to be local without asking the daemon', async () => {
    await notifyLanSettings(true, { networks: DEFAULT_NETWORKS, custom: false });
    await page.getByText('Add a network').click();
    await page.getByPlaceholder('Enter a network, e.g. 192.168.1.0/24').fill('0.0.0.0/0');
    await page.keyboard.press('Enter');
    const error = page.getByText('This network is too broad to be shared outside the tunnel.');
    await expect(error).toBeVisible();

    await page.getByPlaceholder('Enter a network, e.g. 192.168.1.0/24').blur();
    await expect(error).not.toBeVisible();
  });

  test('removes a network and resets a custom list', async () => {
    const custom = [...DEFAULT_NETWORKS, '400::/7'];
    await notifyLanSettings(true, { networks: custom, custom: true });

    const [removed] = await Promise.all([
      util.ipc.settings.setLanNetworks.expect(),
      page.getByText('10.0.0.0/8').locator('..').getByRole('button').click(),
    ]);
    expect(removed).toEqual(custom.filter((network) => network !== '10.0.0.0/8'));

    const [reset] = await Promise.all([
      util.ipc.settings.setLanNetworks.expect(),
      page.getByText('Reset to default').click(),
    ]);
    expect(reset).toBeUndefined();
  });
});
