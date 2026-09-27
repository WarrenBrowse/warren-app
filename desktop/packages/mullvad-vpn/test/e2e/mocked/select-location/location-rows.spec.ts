import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../../src/main/default-settings';
import type { CustomLists, ISettings, Recents } from '../../../../src/shared/daemon-rpc-types';
import { RoutePath } from '../../../../src/shared/routes';
import { mockData } from '../../mock-data';
import { RoutesObjectModel } from '../../route-object-models';
import { MockedTestUtils, startMockedApp } from '../mocked-utils';
import { LIVE_HOST, relayList, serveWarrenNetwork, SHOTS } from '../warren-network-fixture';

// One row anatomy everywhere in the picker (flag, name, load, a quiet lists
// button), and lists that exist only while they hold a place.

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

const recents: Recents = [
  { type: 'singlehop', location: { country: 'fi' } },
  { type: 'singlehop', location: { country: 'fr', city: 'par', hostname: LIVE_HOST } },
  { type: 'singlehop', location: { country: 'ro' } },
];

function settingsWith(customLists: CustomLists, withRecents: Recents | undefined = recents) {
  const settings: ISettings = { ...getDefaultSettings(), customLists, recents: withRecents };
  if ('normal' in settings.relaySettings) {
    settings.relaySettings.normal.location = { only: { country: 'fi' } };
  }
  return settings;
}

async function useSettings(customLists: CustomLists, withRecents?: Recents) {
  await util.ipc.settings[''].notify(settingsWith(customLists, withRecents));
}

const section = (name: string) => page.getByRole('region', { name });
const menuButton = (name: string) => page.getByRole('button', { name: `Open menu for ${name}` });

test.describe('Location rows', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);
    await util.ipc.relays[''].notify({
      relayList,
      wireguardEndpointData: mockData.wireguardEndpointData,
    });
    await serveWarrenNetwork(util);
    await useSettings([]);
    await routes.main.gotoSelectLocation();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('recents look like every other row', async () => {
    await useSettings([]);
    const recent = section('Recents');

    await expect(recent.locator('img[src$="flags/fi.svg"]')).toHaveCount(1);
    await expect(recent.locator('img[src$="flags/fr.svg"]')).toHaveCount(1);
    await expect(recent.getByTestId('exit-load-badge')).toHaveCount(3);
    // A relay recent names where it is.
    await expect(recent.getByText('Paris, France')).toBeVisible();
    await page.screenshot({ path: `${SHOTS}/rows-recents.png` });
  });

  test('the selected row is marked by its colour and its flag, with no check before the name', async () => {
    const selected = page.getByTestId('location-row-lead').filter({ hasText: 'Finland' });
    await expect(selected).toHaveCount(2);
    for (const row of await selected.all()) {
      await expect(row).toHaveAttribute('data-selected', 'true');
      await expect(row.getByTestId('selected-check')).toHaveCount(0);
    }
  });

  test('the lists button stays quiet until the row is pointed at', async () => {
    const button = menuButton('France').last();
    await expect(button).toHaveCSS('opacity', '0');
    await page.getByRole('button', { name: 'Connect to France' }).last().hover();
    await expect(button).toHaveCSS('opacity', '1');
  });

  test('the lists menu adds a place to a list, and removes it from there', async () => {
    await useSettings([{ id: 'work', name: 'Work', locations: [{ country: 'fi' }] }]);

    await page.getByRole('button', { name: 'Connect to France' }).last().hover();
    await menuButton('France').last().click();
    await expect(page.getByRole('button', { name: 'Add France to new list' })).toBeVisible();
    const added = util.ipc.customLists.updateCustomList.expect();
    await page.getByRole('button', { name: 'Add France to Work' }).click();
    expect(await added).toEqual({
      id: 'work',
      name: 'Work',
      locations: [{ country: 'fi' }, { country: 'fr' }],
    });

    await page
      .getByRole('button', { name: 'Connect to Finland' })
      .last()
      .click({ button: 'right' });
    await page.screenshot({ path: `${SHOTS}/rows-lists-menu.png` });
    const removed = util.ipc.customLists.updateCustomList.expect();
    await page.getByRole('button', { name: 'Remove Finland from Work' }).click();
    expect(await removed).toEqual({ id: 'work', name: 'Work', locations: [] });
  });

  test('the lists section shows only the lists that hold a place', async () => {
    await useSettings([
      { id: 'work', name: 'Work', locations: [{ country: 'fi' }, { country: 'ro' }] },
      { id: 'empty', name: 'Empty', locations: [] },
    ]);
    const lists = section('Custom lists');
    await expect(lists.getByRole('button', { name: 'Connect to Work' })).toBeVisible();
    await expect(lists.getByRole('button', { name: 'Connect to Empty' })).toHaveCount(0);
    await expect(lists.getByText('2 locations')).toBeVisible();
    // A list opens on its places.
    await expect(page.getByRole('button', { name: 'Collapse Work' })).toBeVisible();
    await expect(lists.locator('img[src$="flags/ro.svg"]')).toHaveCount(1);
    await expect(lists.getByTestId('exit-load-badge')).toHaveCount(2);
    await page.screenshot({ path: `${SHOTS}/rows-lists.png` });

    await useSettings([{ id: 'empty', name: 'Empty', locations: [] }]);
    await expect(section('Custom lists')).toHaveCount(0);
  });

  test('an empty list is still offered in the lists menu, to be filled again', async () => {
    await useSettings([{ id: 'empty', name: 'Empty', locations: [] }]);
    await page
      .getByRole('button', { name: 'Connect to Romania' })
      .last()
      .click({ button: 'right' });
    await expect(page.getByRole('button', { name: 'Add Romania to Empty' })).toBeVisible();
    await page.keyboard.press('Escape');
  });

  test('recents are hidden while there are none', async () => {
    await useSettings([], []);
    await expect(section('Recents')).toHaveCount(0);
    await expect(section('All locations')).toBeVisible();
    await page.screenshot({ path: `${SHOTS}/rows-plain.png` });
  });
});
