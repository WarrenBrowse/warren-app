import { renderToStaticMarkup } from 'react-dom/server';
import { ServerStyleSheet } from 'styled-components';
import { describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env.platform` at top level.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import { DeviceRevokedMessage } from '../../src/renderer/components/views/device-revoked/DeviceRevokedView';

function render(element: React.ReactElement): string {
  const sheet = new ServerStyleSheet();
  try {
    return renderToStaticMarkup(sheet.collectStyles(element));
  } finally {
    sheet.seal();
  }
}

// warren-core doc 115, section 7.1: a device revoked from another device of
// the account says so, and what brings it back.
describe('the revoked screen', () => {
  it('says the device was disconnected from another one, and how to sign in again', () => {
    const html = render(<DeviceRevokedMessage blocking={false} />);

    expect(html).toContain(
      'This device was disconnected from another device of your account. Sign in with your recovery phrase to use it again.',
    );
    expect(html).not.toContain('Going to login will unblock the Internet on this device.');
  });

  it('adds that going to login unblocks the internet while the tunnel blocks it', () => {
    expect(render(<DeviceRevokedMessage blocking={true} />)).toContain(
      'Going to login will unblock the Internet on this device.',
    );
  });
});
