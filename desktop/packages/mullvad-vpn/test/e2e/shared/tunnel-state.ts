import { expect } from '@playwright/test';
import { Page } from 'playwright';

// The home screen tells a state by its pill and by what its action button does.
interface TunnelStateContent {
  pill: string;
  buttonText?: string;
}

export async function expectDisconnected(page: Page) {
  await expectTunnelState(page, { pill: 'Unprotected', buttonText: 'Connect' });
}

export async function expectConnecting(page: Page) {
  await expectTunnelState(page, { pill: 'Connecting', buttonText: 'Cancel' });
}

export async function expectConnected(page: Page) {
  await expectTunnelState(page, { pill: 'Protected', buttonText: 'Disconnect' });
}

export async function expectDisconnecting(page: Page) {
  await expectTunnelState(page, { pill: 'Connecting', buttonText: 'Connect' });
}

export async function expectError(page: Page) {
  await expectTunnelState(page, { pill: 'Blocked', buttonText: 'Disconnect' });
}

export async function expectTunnelState(page: Page, content: TunnelStateContent) {
  await expect(page.getByTestId('home-pill')).toHaveText(content.pill);
  if (content.buttonText) {
    await expect(page.getByRole('button', { name: content.buttonText, exact: true })).toBeVisible();
  }
}
