import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

const guiSettings = {
  preferredLocale: 'en',
  enableSystemNotifications: true,
  autoConnect: false,
  monochromaticIcon: false,
  startMinimized: false,
  unpinnedWindow: true,
  browsedForSplitTunnelingApplications: [],
  changelogDisplayedForVersion: '',
  updateDismissedForVersion: '',
  onboardingPending: false,
};

// Filing a report is one click from home and one sentence long: the details
// the forum form has are folded away, and a filed report ends on the screen
// that says where its follow-up happens.
test.describe('Report a problem', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await util.ipc.guiSettings[''].notify(guiSettings);
    await routes.main.waitForRoute();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('opens from the home lifebuoy with only the description to fill in', async () => {
    await page.getByTestId('report-problem-button').click();
    await util.expectRoute(RoutePath.reportProblem);

    const send = page.getByRole('button', { name: 'Send the report' });
    await expect(send).toBeDisabled();
    await expect(page.getByText('Where does the problem happen?')).toBeHidden();
    await expect(page.getByText('How often does it happen?')).toBeHidden();

    await page.getByLabel('What happened?').fill('No internet');
    await expect(send).toBeEnabled();
  });

  test('keeps the optional details one click away', async () => {
    await page.getByRole('button', { name: 'More details (optional)' }).click();
    await expect(page.getByText('Where does the problem happen?')).toBeVisible();
    await expect(page.getByText('How often does it happen?')).toBeVisible();
  });

  test('sends the description alone and says where the follow-up happens', async () => {
    await Promise.all([
      util.ipc.forumReport.send.expect({
        kind: 'created',
        topicId: 4242,
        topicUrl: 'https://forum.warrenbrowse.com/t/4242',
        logs: 'attached',
        identity: { handle: 'jugop-lobab-virar', notifySlot: 1 },
        openedInBrowser: true,
      }),
      page.getByRole('button', { name: 'Send the report' }).click(),
    ]);
    await expect(
      page.getByText('Your report is on the forum and your logs reached the support team.'),
    ).toBeVisible();
    await expect(page.getByText('The topic opened in your browser.')).toBeVisible();
    await expect(page.getByText(/The follow-up happens on that forum topic/)).toBeVisible();
    await expect(page.getByRole('button', { name: 'Open the topic' })).toBeVisible();

    await page.getByRole('button', { name: 'Report another problem' }).click();
    await expect(page.getByLabel('What happened?')).toHaveValue('');
  });

  test('lists a filed report with a way to send new logs to it', async () => {
    await util.ipc.guiSettings[''].notify({
      ...guiSettings,
      forumReports: [
        { topicId: 4242, summary: 'No internet', createdAt: '2026-11-27T10:00:00.000Z' },
      ],
    });
    await expect(page.getByText('Your reports')).toBeVisible();
    await Promise.all([
      util.ipc.forumReport.sendLogs.expect({ kind: 'attached' }),
      page.getByRole('button', { name: 'Send new logs' }).click(),
    ]);
    await expect(page.getByText('The new logs reached the support team.')).toBeVisible();
  });
});
