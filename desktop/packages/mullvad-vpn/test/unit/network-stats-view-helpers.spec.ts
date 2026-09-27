import { describe, expect, it } from 'vitest';

import type {
  CityLocation,
  CountryLocation,
  RelayLocation,
} from '../../src/renderer/features/locations/types';
import { loadLevelColors } from '../../src/renderer/lib/foundations/variables/load-color-variables';
import {
  arcDashOffset,
  bandArcPercent,
  holdsSeveralExits,
  liveThresholdNote,
  loadLevelLabel,
  ringGeometry,
  showsAsSelected,
  singleExitHostname,
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

describe('holdsSeveralExits', () => {
  it('is false for a relay, and for a city or a country holding one exit', () => {
    expect(holdsSeveralExits(relay('warren-a'))).toBe(false);
    expect(holdsSeveralExits(city([relay('warren-a')]))).toBe(false);
    expect(holdsSeveralExits(country([city([relay('warren-a')])]))).toBe(false);
  });

  it('is true for a place holding several exits, in one city or across cities', () => {
    expect(holdsSeveralExits(city([relay('warren-a'), relay('warren-b')]))).toBe(true);
    expect(holdsSeveralExits(country([city([relay('warren-a')]), city([relay('warren-b')])]))).toBe(
      true,
    );
  });
});

describe('showsAsSelected', () => {
  it('marks a row selected by itself', () => {
    expect(showsAsSelected({ ...relay('warren-a'), selected: true })).toBe(true);
  });

  it('marks a single-exit place selected when its hidden exit is the selection', () => {
    const chosen = { ...relay('warren-a'), selected: true };

    expect(showsAsSelected(country([city([chosen])]))).toBe(true);
    expect(showsAsSelected(city([chosen]))).toBe(true);
  });

  it('leaves a place with several exits unmarked when only one of them is selected', () => {
    const chosen = { ...relay('warren-a'), selected: true };

    expect(showsAsSelected(city([chosen, relay('warren-b')]))).toBe(false);
  });

  it('leaves an unselected single-exit place unmarked', () => {
    expect(showsAsSelected(country([city([relay('warren-a')])]))).toBe(false);
  });
});

describe('bandArcPercent', () => {
  it('fills the ring of a quiet exit by band, so the shape carries the level too', () => {
    expect(bandArcPercent('low')).toBe(25);
    expect(bandArcPercent('moderate')).toBe(60);
    expect(bandArcPercent('high')).toBe(85);
    expect(bandArcPercent('saturated')).toBe(100);
  });

  it('draws no arc for an unknown band', () => {
    expect(bandArcPercent('unknown')).toBe(0);
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

  it('explains the live threshold with the value the snapshot carries', () => {
    expect(liveThresholdNote(20)).toBe(
      'Live figures appear once 20 people share an exit, so no one’s traffic can be singled out.',
    );
  });
});
