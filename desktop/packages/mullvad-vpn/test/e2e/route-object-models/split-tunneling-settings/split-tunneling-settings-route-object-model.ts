import { Page } from 'playwright';

import { RoutePath } from '../../../../src/shared/routes';
import { type TestUtils } from '../../utils';
import { createSelectors } from './selectors';

export class SplitTunnelingSettingsRouteObjectModel {
  readonly page: Page;
  readonly utils: TestUtils;
  public readonly selectors: ReturnType<typeof createSelectors>;

  constructor(page: Page, utils: TestUtils) {
    this.page = page;
    this.utils = utils;
    this.selectors = createSelectors(page);
  }

  async waitForRoute() {
    await this.utils.expectRoute(RoutePath.splitTunneling);
  }

  // Opens the route of an app that has no rule yet, from the list of apps.
  async openRouteOfNewApp(applicationName: string) {
    await this.selectors.addAppButton().click();
    await this.selectors.searchInput().fill(applicationName);
    await this.selectors.appToAdd(applicationName).first().click();
  }

  async openRouteOfRule(applicationName: string) {
    await this.selectors.rule(applicationName).click();
  }
}
