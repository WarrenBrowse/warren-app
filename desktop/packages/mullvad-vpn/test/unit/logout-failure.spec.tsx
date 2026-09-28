import { renderToStaticMarkup } from 'react-dom/server';
import { ServerStyleSheet } from 'styled-components';
import { describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env.platform` at top level.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import { LogoutFailure, logoutFailureMessage } from '../../src/renderer/components/LogoutFailure';

function render(element: React.ReactElement): string {
  const sheet = new ServerStyleSheet();
  try {
    return renderToStaticMarkup(sheet.collectStyles(element));
  } finally {
    sheet.seal();
  }
}

describe('the logout failure', () => {
  it('says the tunnel did not come down in time and the account is kept', () => {
    expect(logoutFailureMessage('tunnel-still-up')).to.equal(
      'Could not log out: the VPN did not disconnect in time. You are still logged in.',
    );
  });

  it('says a logout that failed otherwise left the account logged in', () => {
    expect(logoutFailureMessage('failed')).to.equal('Could not log out. You are still logged in.');
  });

  it('shows nothing once the logout went through, or before any', () => {
    expect(logoutFailureMessage('logged-out')).toBeUndefined();
    expect(render(<LogoutFailure result="logged-out" onRetry={() => {}} />)).to.equal('');
    expect(render(<LogoutFailure result={undefined} onRetry={() => {}} />)).to.equal('');
  });

  it('shows the reason as an alert with a retry action', () => {
    const html = render(<LogoutFailure result="tunnel-still-up" onRetry={() => {}} />);

    expect(html).to.contain('role="alert"');
    expect(html).to.contain('the VPN did not disconnect in time');
    expect(html).to.contain('Try again');
  });
});
