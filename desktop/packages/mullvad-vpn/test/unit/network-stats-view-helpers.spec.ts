import { describe, expect, it } from 'vitest';

import type {
  CityLocation,
  CountryLocation,
  RelayLocation,
} from '../../src/renderer/features/locations/types';
import { loadLevelColors } from '../../src/renderer/lib/foundations/variables/load-color-variables';
import {
  arcDashOffset,
  formatSnapshotAge,
  liveThresholdNote,
  loadLevelLabel,
  relayPlacesByHostname,
  ringGeometry,
  singleExitHostname,
  sparklinePaths,
  tweenAt,
} from '../../src/renderer/lib/network-stats/helpers';

describe('ring geometry', () => {
  it('fits the stroke inside the requested diameter', () => {
    const ring = ringGeometry(24, 4);

    expect(ring.center).toBe(12);
    expect(ring.radius).toBe(10);
    expect(ring.circumference).toBeCloseTo(2 * Math.PI * 10);
  });

  it('draws the arc as the share of the circumference the load covers', () => {
    expect(arcDashOffset(0, 100)).toBe(100);
    expect(arcDashOffset(37, 100)).toBeCloseTo(63);
    expect(arcDashOffset(100, 100)).toBe(0);
  });

  it('never draws past a full turn or backwards', () => {
    expect(arcDashOffset(250, 100)).toBe(0);
    expect(arcDashOffset(-5, 100)).toBe(100);
  });
});

describe('sparklinePaths', () => {
  it('draws nothing without a point', () => {
    expect(sparklinePaths([], 100, 20)).toBeUndefined();
  });

  it('marks a single point at the right edge without inventing a line', () => {
    const paths = sparklinePaths([5], 100, 20);

    expect(paths?.line).toBe('');
    expect(paths?.last.x).toBe(100 - paths!.inset);
  });

  it('spans the width and puts the highest value at the top', () => {
    const paths = sparklinePaths([0, 10, 5], 100, 20)!;
    const inset = paths.inset;

    expect(paths.line).toBe(
      `M${inset},${20 - inset} L50,${inset} L${100 - inset},${10}`.replace(/\s+/g, ' '),
    );
    expect(paths.last).toEqual({ x: 100 - inset, y: 10 });
    expect(paths.area.startsWith(paths.line)).toBe(true);
  });

  it('draws a flat series along the middle instead of dividing by zero', () => {
    const paths = sparklinePaths([7, 7], 100, 20)!;

    expect(paths.last.y).toBe(10);
  });
});

describe('tweenAt', () => {
  it('starts at the previous value and lands on the new one', () => {
    expect(tweenAt(10, 50, 0, 800)).toBe(10);
    expect(tweenAt(10, 50, 800, 800)).toBe(50);
    expect(tweenAt(10, 50, 5_000, 800)).toBe(50);
  });

  it('eases out: past halfway by the middle of the duration', () => {
    expect(tweenAt(0, 100, 400, 800)).toBeGreaterThan(50);
    expect(tweenAt(0, 100, 400, 800)).toBeLessThan(100);
  });
});

function relay(hostname: string, city = 'Paris', country = 'fr'): RelayLocation {
  return {
    type: 'relay',
    label: hostname,
    active: true,
    expanded: false,
    selected: false,
    details: { country, city, hostname },
  };
}

function city(relays: RelayLocation[]): CityLocation {
  return {
    type: 'city',
    label: 'Paris',
    active: true,
    expanded: false,
    selected: false,
    details: { country: 'fr', city: 'par' },
    relays,
  };
}

function country(cities: CityLocation[]): CountryLocation {
  return {
    type: 'country',
    label: 'France',
    active: true,
    expanded: false,
    selected: false,
    details: { country: 'fr' },
    cities,
  };
}

describe('singleExitHostname', () => {
  it('names the relay of a relay row', () => {
    expect(singleExitHostname(relay('warren-a'))).toBe('warren-a');
  });

  it('names the only exit of a city or a country', () => {
    expect(singleExitHostname(city([relay('warren-a')]))).toBe('warren-a');
    expect(singleExitHostname(country([city([relay('warren-a')])]))).toBe('warren-a');
  });

  it('names nothing for a place with several exits', () => {
    expect(singleExitHostname(city([relay('warren-a'), relay('warren-b')]))).toBeUndefined();
    expect(
      singleExitHostname(country([city([relay('warren-a')]), city([relay('warren-b')])])),
    ).toBeUndefined();
  });
});

describe('relayPlacesByHostname', () => {
  it('reads each exit place from the signed relay list', () => {
    const places = relayPlacesByHostname([
      {
        name: 'France',
        code: 'fr',
        cities: [
          {
            name: 'Paris',
            code: 'par',
            latitude: 0,
            longitude: 0,
            relays: [{ hostname: 'warren-a' } as never],
          },
        ],
      },
    ]);

    expect(places.get('warren-a')).toEqual({ country: 'France', city: 'Paris' });
  });
});

describe('load colours', () => {
  it('colours each band from the palette, and anything else neutral', () => {
    expect(loadLevelColors.low).toBe('green');
    expect(loadLevelColors.moderate).toBe('yellow');
    expect(loadLevelColors.high).toBe('orange');
    expect(loadLevelColors.saturated).toBe('red');
    expect(loadLevelColors.unknown).toBe('whiteOnDarkBlue40');
  });
});

describe('labels', () => {
  it('names each band', () => {
    expect(loadLevelLabel('low')).toBe('Low load');
    expect(loadLevelLabel('saturated')).toBe('Saturated');
    expect(loadLevelLabel('unknown')).toBe('Load unknown');
  });

  it('says how old the snapshot is in the largest whole unit', () => {
    expect(formatSnapshotAge(42)).toBe('Updated 42 s ago');
    expect(formatSnapshotAge(150)).toBe('Updated 2 min ago');
    expect(formatSnapshotAge(7_300)).toBe('Updated 2 h ago');
  });

  it('explains the live threshold with the value the snapshot carries', () => {
    expect(liveThresholdNote(20)).toBe(
      "Live figures appear once 20 people share an exit, so no one's traffic can be singled out.",
    );
  });
});
