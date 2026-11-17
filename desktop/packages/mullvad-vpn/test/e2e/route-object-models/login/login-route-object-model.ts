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

  // Types back the words the check asks for, read from each field's label
  // ("Word 7"), taken from `phrase`; `wrong` replaces the last one.
  async answerBackupCheck(phrase: string, wrong = false) {
    const words = phrase.split(' ');
    for (let index = 0; index < 3; index++) {
      const field = this.selectors.backupCheckWord(index);
      const label = (await field.getAttribute('aria-label')) ?? '';
      const position = Number(/(\d+)/.exec(label)?.[1]);
      const answer = wrong && index === 2 ? 'zoo' : words[position - 1];
      await field.fill(answer);
    }
    await this.selectors.backupCheckContinueButton().click();
  }

  async discardNewAccount() {
    await this.selectors.backupBackButton().click();
    await this.selectors.backupDiscardConfirmButton().click();
  }

  async startRestore() {
    await this.selectors.restoreButton().click();
  }

  async leaveRestore() {
    await this.selectors.restoreBackButton().click();
  }

  async fillRecoveryPhrase(phrase: string) {
    await this.selectors.restoreInput().fill(phrase);
  }

  async submitRestore() {
    await this.selectors.restoreSubmitButton().click();
  }
}
