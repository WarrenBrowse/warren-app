import { Page } from 'playwright';

import { RoutePath } from '../../../../src/shared/routes';
import { TestUtils } from '../../utils';
import { createSelectors } from './selectors';

export class LoginRouteObjectModel {
  readonly selectors: ReturnType<typeof createSelectors>;

  constructor(
    private readonly page: Page,
    private readonly utils: TestUtils,
  ) {
    this.selectors = createSelectors(this.page);
  }

  async waitForRoute() {
    await this.utils.expectRoute(RoutePath.login);
  }

  async createNewAccount() {
    await this.selectors.createAccountButton().click();
  }

  async confirmBackup() {
    await this.selectors.backupConfirmCheckbox().click();
    await this.selectors.backupContinueButton().click();
  }

  async startRestore() {
    await this.selectors.restoreButton().click();
  }

  async fillRecoveryPhrase(phrase: string) {
    await this.selectors.restoreInput().fill(phrase);
  }

  async submitRestore() {
    await this.selectors.restoreSubmitButton().click();
  }
}
