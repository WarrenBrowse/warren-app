import { Page } from 'playwright';

export const createSelectors = (page: Page) => ({
  createNewAccountButton: () => page.getByRole('button', { name: 'Create a new account' }),
  createNewAccountMessage: () => page.getByText('Do you want to create a new account?'),
  confirmCreateNewAccountButton: () => page.getByRole('button', { name: 'Create new account' }),

  accountHistoryItemButton: () => page.getByRole('button', { name: 'Login with public key' }),
  clearAccountHistory: () => page.getByRole('button', { name: 'Forget public key' }),
  clearAccountHistoryMessage: () => page.getByText('Do you want to remove the saved public key?'),
  confirmClearAccountHistoryButton: () => page.getByRole('button', { name: 'Remove' }),

  cancelDialogButton: () => page.getByRole('button', { name: 'Cancel' }),
  loginInput: () => page.getByTestId('subtitle'),
  loginButton: () => page.getByRole('button', { name: 'Login', exact: true }),
  header: () => page.getByRole('heading', { level: 1 }),
});
