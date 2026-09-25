import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env.platform` when it is
// imported, so the stub has to exist before the imports below run.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import { AppCountryRow } from '../../src/renderer/components/views/split-tunneling/components/country-per-app/AppCountryRow';
import { ISplitTunnelingApplication } from '../../src/shared/application-types';

const application = (
  routingLimitation?: ISplitTunnelingApplication['routingLimitation'],
): ISplitTunnelingApplication => ({
  absolutepath: '/usr/share/applications/org.gimp.GIMP.desktop',
  name: 'GIMP',
  deletable: false,
  routingLimitation,
});

const render = (app: ISplitTunnelingApplication) =>
  renderToStaticMarkup(<AppCountryRow application={app} onPick={() => undefined} />);

describe('AppCountryRow, one app of the Country per app tab', () => {
  it('offers a country for an app whose program it can name', () => {
    const markup = render(application());

    expect(markup).not.toMatch(/<button[^>]*disabled/);
  });

  it('says why a sandboxed app takes no country, and offers none', () => {
    const flatpak = render(application('flatpak'));
    const snap = render(application('snap'));

    expect(flatpak).toContain('Flatpak apps cannot use a country yet');
    expect(snap).toContain('Snap apps cannot use a country yet');
    expect(flatpak).toMatch(/<button[^>]*disabled/);
  });

  it('points to the program behind a launcher script', () => {
    const markup = render(application('script'));

    expect(markup).toContain('Opens through a script: pick its program with Find another app');
    expect(markup).toMatch(/<button[^>]*disabled/);
  });
});
