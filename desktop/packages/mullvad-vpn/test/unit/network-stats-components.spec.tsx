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

import { ExitLoadSummary, LoadRing, Sparkline } from '../../src/renderer/components/network-stats';
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
