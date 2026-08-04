import { Page } from 'playwright';

export const createSelectors = (page: Page) => ({
  header: () => page.getByRole('heading', { level: 1 }),

  // Welcome (pick) step.
  createAccountButton: () => page.getByTestId('login-create'),
  restoreButton: () => page.getByTestId('login-restore'),

  // Backup step (shown after creating a new account).
  mnemonicGrid: () => page.getByTestId('login-mnemonic-grid'),
  backupConfirmCheckbox: () => page.getByRole('checkbox'),
  backupContinueButton: () => page.getByTestId('login-backup-continue'),

  // Restore step.
  restoreInput: () => page.getByTestId('login-restore-input'),
  restoreSubmitButton: () => page.getByTestId('login-restore-submit'),
});
