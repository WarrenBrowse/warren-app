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

import { ExitLoadBadge, LoadRing } from '../../src/renderer/components/network-stats';
import { NetworkStats, parseNetworkStats } from '../../src/shared/network-stats';

const STATS = parseNetworkStats(
  readFileSync(path.resolve(__dirname, '../fixtures/network-stats-v1.json'), 'utf8'),
) as NetworkStats;
const [LIVE_EXIT, QUIET_EXIT] = STATS.exits;

// The text a sighted reader sees, without markup or accessibility labels.
function visibleText(html: string): string {
  return html
    .replace(/<svg.*?<\/svg>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .trim();
}

describe('LoadRing', () => {
  it('draws the arc over a track', () => {
    const html = renderToStaticMarkup(<LoadRing level="low" percent={37} />);

    expect(html).toContain('data-testid="load-ring-arc"');
    expect(html).toContain('data-testid="load-ring-track"');
  });

  it('draws no arc at zero', () => {
    expect(renderToStaticMarkup(<LoadRing level="low" percent={0} />)).not.toContain(
      'load-ring-arc',
    );
  });

  it('colours the arc from the band, and grey when muted', () => {
    expect(renderToStaticMarkup(<LoadRing level="saturated" percent={95} />)).toContain(
      'var(--color-red)',
    );
    expect(renderToStaticMarkup(<LoadRing level="saturated" percent={95} muted />)).not.toContain(
      'var(--color-red)',
    );
  });
});

describe('ExitLoadBadge', () => {
  it('shows a quiet exit as a band-filled ring and the live threshold, with no word or percentage', () => {
    const html = renderToStaticMarkup(
      <ExitLoadBadge exit={QUIET_EXIT} stats={STATS} locale="en" />,
    );

    expect(visibleText(html)).toBe('< 20');
    expect(html).toContain('load-ring-arc');
    expect(html).toContain('aria-label="Moderate load, &lt; 20 people"');
  });

  it('shows a live exit as its percentage and its floored people count', () => {
    const html = renderToStaticMarkup(<ExitLoadBadge exit={LIVE_EXIT} stats={STATS} locale="en" />);

    expect(visibleText(html)).toBe('37% 40+');
    expect(html).toContain('aria-label="Load 37%, 40+ people"');
  });

  it('adds the download rate of a live exit when asked', () => {
    const html = renderToStaticMarkup(
      <ExitLoadBadge exit={LIVE_EXIT} stats={STATS} locale="en" throughput />,
    );

    expect(visibleText(html)).toBe('37% 40+ 300 Mbit/s');
  });

  it('never shows a rate for a quiet exit', () => {
    const html = renderToStaticMarkup(
      <ExitLoadBadge exit={QUIET_EXIT} stats={STATS} locale="en" throughput />,
    );

    expect(visibleText(html)).toBe('< 20');
  });

  it('shows an offline exit as offline, without a people count', () => {
    const html = renderToStaticMarkup(
      <ExitLoadBadge exit={{ ...LIVE_EXIT, online: false }} stats={STATS} locale="en" />,
    );

    expect(visibleText(html)).toBe('Offline');
    expect(html).not.toContain('load-ring-arc');
  });
});
