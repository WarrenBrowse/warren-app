import { expect, test } from '@playwright/test';
import { execSync } from 'child_process';
import { Page } from 'playwright';

import { RoutePath } from '../../../../src/shared/routes';
import { RoutesObjectModel } from '../../route-object-models';
import { TestUtils } from '../../utils';
import { startInstalledApp } from '../installed-utils';

// Windows and macOS only. This test expects the daemon to be logged in, with no app routing rule.

interface Application {
  name: string;
  filename?: string;
}

const application: Application =
  process.platform === 'win32'
    ? { name: 'microsoft edge', filename: 'msedge.exe' }
    : { name: 'launchpad' };

let page: Page;
let util: TestUtils;
let routes: RoutesObjectModel;

test.describe('App routing', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startInstalledApp());
    routes = new RoutesObjectModel(page, util);

    await util.expectRoute(RoutePath.main);

    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('opens with no rule and the VPN as the default', async () => {
    await expect(routes.splitTunnelingSettings.selectors.heading()).toBeVisible();
    await expect(routes.splitTunnelingSettings.selectors.rules()).not.toBeVisible();
    await expect(
      routes.splitTunnelingSettings.selectors.defaultRoute('Through the VPN'),
    ).toHaveAttribute('aria-pressed', 'true');
    expect(getDaemonExcludedApplications()).toHaveLength(0);
  });

  test(`sends ${application.name} outside the VPN, which turns the bypass on`, async () => {
    await routes.splitTunnelingSettings.openRouteOfNewApp(application.name);
    await routes.splitTunnelingSettings.selectors.routeOption('outside').click();
    await expect(routes.splitTunnelingSettings.selectors.routeOption('outside')).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await routes.splitTunnelingSettings.selectors.doneButton().click();

    await expect(routes.splitTunnelingSettings.selectors.rule(application.name)).toBeVisible();
    expect(getDaemonExcludedApplications()).toHaveLength(1);
    expect(isExcludedInDaemon(application)).toBeTruthy();
  });

  test(`brings ${application.name} back to the VPN with its rule`, async () => {
    await routes.splitTunnelingSettings.openRouteOfRule(application.name);
    await routes.splitTunnelingSettings.selectors.removeRuleButton().click();

    await expect(routes.splitTunnelingSettings.selectors.rules()).not.toBeVisible();
    expect(getDaemonExcludedApplications()).toHaveLength(0);
    expect(isExcludedInDaemon(application)).toBeFalsy();
  });
});

function getDaemonExcludedApplications() {
  const output = execSync('mullvad split-tunnel get').toString().trim().split('\n');
  return output.slice(output.indexOf('Excluded applications:') + 1).filter(Boolean);
}

function isExcludedInDaemon(application: Application): boolean {
  return !!getDaemonExcludedApplications().find((splitApp) =>
    splitApp.toLowerCase().includes(application.filename ?? application.name),
  );
}
