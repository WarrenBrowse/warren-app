import { readFileSync } from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import {
  exitDisplayMode,
  exitUsersLabel,
  formatBitsPerSecond,
  formatBytes,
  formatPercent,
  joinExitsByHostname,
  NetworkStats,
  parseNetworkStats,
  pollIntervalMs,
  snapshotAgeSecs,
  snapshotIsStale,
  uptimeDays,
} from '../../src/shared/network-stats';

// A verbatim copy of warren-contract's frozen `network-stats-v1.json`, the
// wire example every client of `GET /v1/network/stats` is written against.
const FIXTURE = readFileSync(path.resolve(__dirname, '../fixtures/network-stats-v1.json'), 'utf8');

function fixture(): NetworkStats {
  const parsed = parseNetworkStats(FIXTURE);
  if (!parsed) {
    throw new Error('the frozen fixture must parse');
  }
  return parsed;
}

function withJson(edit: (json: Record<string, unknown>) => void): string {
  const json = JSON.parse(FIXTURE) as Record<string, unknown>;
  edit(json);
  return JSON.stringify(json);
}

describe('parseNetworkStats', () => {
  it('reads the frozen fixture', () => {
    const stats = fixture();

    expect(stats.generatedAt).toBe(1790000000);
    expect(stats.windowSecs).toBe(60);
    expect(stats.exitUsersRounding).toBe(5);
    expect(stats.exitLiveThreshold).toBe(20);
    expect(stats.users).toEqual({ accountsTotal: 1234, subscribersActive: 987, connected: 57 });
    expect(stats.fleet.exitsOnline).toBe(2);
    expect(stats.fleet.transferred24hBytes).toBe(9000000000000);
    expect(stats.exits).toHaveLength(2);
    expect(stats.exits[0]).toMatchObject({
      exitId: 'ab'.repeat(16),
      name: 'fr-par-h1b',
      live: true,
      connected: 40,
      loadPercent: 37,
      loadLevel: 'low',
      loadDriver: 'bandwidth',
      uptimeSecs: 172800,
    });
    expect(stats.exits[0].history[0]).toEqual({
      t: 1789999940,
      connected: 40,
      throughputBps: 310000000,
      loadPercent: 35,
    });
    expect(stats.history[0]).toEqual({ t: 1789999100, connected: 55, throughputBps: 420000000 });
  });

  it('leaves the optional figures of a band-only exit absent rather than zero', () => {
    const quiet = fixture().exits[1];

    expect(quiet.live).toBe(false);
    expect(quiet.loadLevel).toBe('moderate');
    expect(quiet.loadPercent).toBeUndefined();
    expect(quiet.capacityBps).toBeUndefined();
    expect(quiet.name).toBeUndefined();
  });

  it('ignores fields it does not know', () => {
    const json = withJson((json) => {
      json.brand_new_block = { anything: true };
      (json.exits as Array<Record<string, unknown>>)[0].future_field = 'x';
    });

    expect(parseNetworkStats(json)?.exits).toHaveLength(2);
  });

  it('maps an unknown load band or driver to unknown', () => {
    const json = withJson((json) => {
      const exit = (json.exits as Array<Record<string, unknown>>)[0];
      exit.load_level = 'molten';
      exit.load_driver = 'disk';
    });

    const exit = parseNetworkStats(json)!.exits[0];
    expect(exit.loadLevel).toBe('unknown');
    expect(exit.loadDriver).toBe('unknown');
  });

  it('reads an absent load band as unknown', () => {
    const json = withJson((json) => {
      delete (json.exits as Array<Record<string, unknown>>)[1].load_level;
    });

    expect(parseNetworkStats(json)!.exits[1].loadLevel).toBe('unknown');
  });

  it('drops a malformed exit and keeps the others', () => {
    const json = withJson((json) => {
      (json.exits as Array<Record<string, unknown>>)[1].exit_id = 'not-hex';
    });

    const exits = parseNetworkStats(json)!.exits;
    expect(exits.map((exit) => exit.exitId)).toEqual(['ab'.repeat(16)]);
  });

  it('refuses another schema version', () => {
    expect(parseNetworkStats(withJson((json) => (json.version = 2)))).toBeUndefined();
  });

  it('refuses a snapshot without its window', () => {
    expect(parseNetworkStats(withJson((json) => delete json.window_secs))).toBeUndefined();
  });

  it('refuses a body that is not a JSON object', () => {
    expect(parseNetworkStats('')).toBeUndefined();
    expect(parseNetworkStats('[1]')).toBeUndefined();
    expect(parseNetworkStats('<html>')).toBeUndefined();
  });

  it('clamps a percentage into 0..100', () => {
    const json = withJson((json) => {
      (json.exits as Array<Record<string, unknown>>)[0].load_percent = 250;
    });

    expect(parseNetworkStats(json)!.exits[0].loadPercent).toBe(100);
  });
});

describe('exit display decisions', () => {
  it('shows a live exit with its figures', () => {
    expect(exitDisplayMode(fixture().exits[0])).toBe('live');
  });

  it('shows a quiet exit as its band only', () => {
    expect(exitDisplayMode(fixture().exits[1])).toBe('band');
  });

  it('shows an offline exit as offline whatever else it carries', () => {
    expect(exitDisplayMode({ ...fixture().exits[0], online: false })).toBe('offline');
  });

  it('labels a band-only exit with the live threshold', () => {
    const stats = fixture();
    expect(exitUsersLabel(stats.exits[1], stats)).toBe('< 20');
  });

  it('labels a live exit count as a floor', () => {
    const stats = fixture();
    expect(exitUsersLabel(stats.exits[0], stats)).toBe('40+');
  });

  it('labels a live exit under one rounding step as less than the step', () => {
    const stats = fixture();
    expect(exitUsersLabel({ ...stats.exits[0], connected: 0 }, stats)).toBe('< 5');
  });
});

describe('joinExitsByHostname', () => {
  it('keys each exit by the relay list hostname the daemon joined it to', () => {
    const stats = fixture();

    const joined = joinExitsByHostname(stats, [
      { exitId: 'cd'.repeat(16), hostname: 'warren-cdcd' },
      { exitId: 'ab'.repeat(16), hostname: 'warren-abab' },
      { exitId: 'ef'.repeat(16), hostname: 'warren-efef' },
    ]);

    expect(joined.get('warren-abab')?.name).toBe('fr-par-h1b');
    expect(joined.get('warren-cdcd')?.city).toBe('Bucharest');
    expect(joined.has('warren-efef')).toBe(false);
  });
});

describe('formatBitsPerSecond', () => {
  it('uses SI units with one decimal under ten', () => {
    expect(formatBitsPerSecond(4_200_000, 'en')).toBe('4.2 Mbit/s');
  });

  it('drops the decimal from ten up', () => {
    expect(formatBitsPerSecond(300_000_000, 'en')).toBe('300 Mbit/s');
  });

  it('moves to the next unit when rounding reaches a thousand', () => {
    expect(formatBitsPerSecond(999_700, 'en')).toBe('1.0 Mbit/s');
  });

  it('does not show a decimal that rounds to ten', () => {
    expect(formatBitsPerSecond(9_960_000, 'en')).toBe('10 Mbit/s');
  });

  it('keeps plain bits without a decimal', () => {
    expect(formatBitsPerSecond(950, 'en')).toBe('950 bit/s');
  });

  it('follows the locale decimal separator', () => {
    expect(formatBitsPerSecond(1_500_000_000, 'fr')).toBe('1,5 Gbit/s');
  });
});

describe('formatBytes', () => {
  it('uses SI units', () => {
    expect(formatBytes(9_000_000_000_000, 'en')).toBe('9.0 TB');
    expect(formatBytes(512, 'en')).toBe('512 B');
    expect(formatBytes(42_000_000, 'en')).toBe('42 MB');
  });
});

describe('formatPercent', () => {
  it('follows the locale', () => {
    expect(formatPercent(37, 'en')).toBe('37%');
    // French sets a no-break space before the sign; which one depends on the
    // ICU version, so Node and Chromium may differ.
    expect(formatPercent(37, 'fr')).toMatch(/^37\s%$/);
  });
});

describe('uptimeDays', () => {
  it('counts whole days', () => {
    expect(uptimeDays(172_800)).toBe(2);
    expect(uptimeDays(86_399)).toBe(0);
  });
});

describe('freshness', () => {
  const stats = { generatedAt: 1_000, windowSecs: 60 };

  it('measures the age of a snapshot from the moment its window closed', () => {
    expect(snapshotAgeSecs(stats, 1_042_500)).toBe(42);
  });

  it('never reports a negative age when the local clock runs behind', () => {
    expect(snapshotAgeSecs(stats, 900_000)).toBe(0);
  });

  it('calls a snapshot stale once it is older than three windows', () => {
    expect(snapshotIsStale(stats, 1_180_000)).toBe(false);
    expect(snapshotIsStale(stats, 1_181_000)).toBe(true);
  });
});

describe('pollIntervalMs', () => {
  it('polls once per window', () => {
    expect(pollIntervalMs(60)).toBe(60_000);
  });

  it('keeps the window inside the range the server accepts', () => {
    expect(pollIntervalMs(1)).toBe(30_000);
    expect(pollIntervalMs(100_000)).toBe(3_600_000);
  });
});
