import { type Page } from 'playwright';

export const createSelectors = (page: Page) => ({
  heading: () => page.getByRole('heading', { level: 1, name: 'App routing' }),
  defaultRoute: (name: 'Through the VPN' | 'Outside the VPN') =>
    page.getByRole('group', { name: 'Other apps go' }).getByRole('button', { name }),
  addAppButton: () => page.getByRole('button', { name: 'Add an app' }),
  addAppScreen: () => page.getByTestId('add-app-screen'),
  appToAdd: (applicationName: string) =>
    page.getByTestId('add-app-screen').getByRole('button', { name: applicationName }),
  rules: () => page.getByTestId('app-rules'),
  rule: (applicationName: string) =>
    page
      .getByTestId('app-rules')
      .getByRole('button', { name: new RegExp(`^${applicationName}`, 'i') }),
  routeScreen: () => page.getByTestId('route-screen'),
  routeOption: (option: 'vpn' | 'country' | 'outside') => page.getByTestId(`route-${option}`),
  removeRuleButton: () => page.getByRole('button', { name: 'Remove the rule' }),
  doneButton: () => page.getByRole('button', { name: 'Done' }),
  findAnotherAppButton: () => page.getByRole('button', { name: 'Find another app' }),
  searchInput: () => page.getByPlaceholder('Search for...'),
});
