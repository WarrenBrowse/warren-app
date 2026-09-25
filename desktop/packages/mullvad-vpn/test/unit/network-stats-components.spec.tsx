import { readFileSync } from 'fs';
import path from 'path';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env` at import time.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

// The views read the locale from the store and nothing else. react-redux
// resolves another React copy than the renderer's under Node, so the hook is
// stubbed rather than given a Provider.
vi.mock('../../src/renderer/redux/store', () => ({
  useSelector: (select: (state: unknown) => unknown) => select({ userInterface: { locale: 'en' } }),
}));

import { ExitLoadSummary, LoadRing, Sparkline } from '../../src/renderer/components/network-stats';
import { ExitCard, FleetCard } from '../../src/renderer/components/views/network/components';
import { NetworkStats, parseNetworkStats } from '../../src/shared/network-stats';

const STATS = parseNetworkStats(
  readFileSync(path.resolve(__dirname, '../fixtures/network-stats-v1.json'), 'utf8'),
) as NetworkStats;
const [LIVE_EXIT, QUIET_EXIT] = STATS.exits;

// What the people pill itself says, apart from the accessibility label.
function pillText(html: string): string | undefined {
  return /data-testid="users-pill"[^>]*>(?:<svg.*?<\/svg>)?(.*?)<\/span>/.exec(html)?.[1];
}

describe('LoadRing', () => {
  it('draws a band-only ring as one full circle with no arc', () => {
    const html = renderToStaticMarkup(<LoadRing size="small" level="moderate" />);

    expect(html).toContain('data-testid="load-ring-band"');
    expect(html).not.toContain('load-ring-arc');
  });

  it('draws a live ring as an arc over a track', () => {
    const html = renderToStaticMarkup(<LoadRing size="small" level="low" percent={37} />);

    expect(html).toContain('data-testid="load-ring-arc"');
    expect(html).not.toContain('load-ring-band');
  });

  it('draws no arc at zero load', () => {
    const html = renderToStaticMarkup(<LoadRing size="small" level="low" percent={0} />);

    expect(html).not.toContain('load-ring-arc');
  });

  it('colours the ring from the band', () => {
    expect(
      renderToStaticMarkup(<LoadRing size="small" level="saturated" percent={95} />),
    ).toContain('var(--color-red)');
  });
});

describe('ExitLoadSummary', () => {
  it('shows a quiet exit as its band name and the live threshold, without a percentage', () => {
    const html = renderToStaticMarkup(
      <ExitLoadSummary exit={QUIET_EXIT} stats={STATS} ringSize="small" locale="en" />,
    );

    expect(html).toContain('Moderate load');
    expect(pillText(html)).toBe('&lt; 20');
    expect(html).not.toContain('%');
  });

  it('shows a live exit as its percentage and its floored people count', () => {
    const html = renderToStaticMarkup(
      <ExitLoadSummary exit={LIVE_EXIT} stats={STATS} ringSize="small" locale="en" />,
    );

    expect(html).toContain('37%');
    expect(pillText(html)).toBe('40+');
    expect(html).toContain('load-ring-arc');
  });

  it('shows an offline exit as offline, without a people count', () => {
    const html = renderToStaticMarkup(
      <ExitLoadSummary
        exit={{ ...LIVE_EXIT, online: false }}
        stats={STATS}
        ringSize="small"
        locale="en"
      />,
    );

    expect(html).toContain('Offline');
    expect(html).not.toContain('users-pill');
  });
});

describe('Sparkline', () => {
  it('renders nothing without history', () => {
    expect(renderToStaticMarkup(<Sparkline values={[]} width={100} height={20} />)).toBe('');
  });

  it('emphasises the newest point', () => {
    const html = renderToStaticMarkup(<Sparkline values={[1, 3, 2]} width={100} height={20} />);

    expect(html).toContain('<circle');
  });
});

function withStore(element: React.ReactElement): string {
  return renderToStaticMarkup(element);
}

describe('FleetCard', () => {
  it('colours the fleet ring by the fleet load band', () => {
    const html = withStore(<FleetCard stats={STATS} stale={false} />);

    expect(html).toContain('stroke:var(--color-green)');
  });

  it('draws the fleet ring neutral when the server sends no band', () => {
    const stats = { ...STATS, fleet: { ...STATS.fleet, loadLevel: 'unknown' as const } };

    const html = withStore(<FleetCard stats={stats} stale={false} />);

    expect(html).toContain('stroke:var(--color-white-on-dark-blue40)');
  });
});

describe('ExitCard', () => {
  it('says nothing about uptime when the server withholds it', () => {
    const exit = { ...LIVE_EXIT, uptimeSecs: undefined };

    const html = withStore(<ExitCard exit={exit} stats={STATS} stale={false} />);

    expect(html).not.toMatch(/Up (\d|less)/);
  });

  it('shows the uptime in whole days when a server publishes it', () => {
    const html = withStore(<ExitCard exit={LIVE_EXIT} stats={STATS} stale={false} />);

    expect(html).toContain('Up 2 days');
  });
});
