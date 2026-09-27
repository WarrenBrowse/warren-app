import { describe, expect, it } from 'vitest';

import {
  listHolds,
  listsWithLocations,
  withoutCustomList,
} from '../../src/renderer/features/custom-lists/utils/membership';
import type { ICustomList } from '../../src/shared/daemon-rpc-types';

const work: ICustomList = {
  id: 'work',
  name: 'Work',
  locations: [{ country: 'fi' }, { country: 'fr', city: 'par', hostname: 'warren-a' }],
};
const empty: ICustomList = { id: 'empty', name: 'Empty', locations: [] };

describe('listHolds', () => {
  it('finds a location the list holds', () => {
    expect(listHolds(work, { country: 'fi' })).toBe(true);
    expect(listHolds(work, { country: 'fr', city: 'par', hostname: 'warren-a' })).toBe(true);
  });

  it('does not take a country for the relay the list holds inside it', () => {
    expect(listHolds(work, { country: 'fr' })).toBe(false);
  });

  it('finds a row shown inside a list, which carries that list id', () => {
    expect(listHolds(work, { country: 'fi', customList: 'work' })).toBe(true);
  });
});

describe('withoutCustomList', () => {
  it('drops the list id a row inside a list carries, and keeps the place', () => {
    expect(withoutCustomList({ country: 'fr', city: 'par', customList: 'work' })).toEqual({
      country: 'fr',
      city: 'par',
    });
  });
});

describe('listsWithLocations', () => {
  it('keeps only the lists that hold something', () => {
    expect(listsWithLocations([work, empty])).toEqual([work]);
    expect(listsWithLocations([empty])).toEqual([]);
  });
});
