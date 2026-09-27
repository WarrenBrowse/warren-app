import {
  compareRelayLocationGeographical,
  type ICustomList,
  type RelayLocationGeographical,
} from '../../../../shared/daemon-rpc-types';

/** Whether `list` holds this exact place: a country is not the relay held inside it. */
export function listHolds(list: ICustomList, location: RelayLocationGeographical): boolean {
  return list.locations.some((listed) => compareRelayLocationGeographical(listed, location));
}

/**
 * The place alone. A row shown inside a list carries that list's id, which
 * must not follow the place into another list.
 */
export function withoutCustomList(location: RelayLocationGeographical): RelayLocationGeographical {
  const { customList: _customList, ...place } = location;
  return place;
}

/** A list exists to hold places: an empty one is not shown. */
export function listsWithLocations<T extends { locations: readonly unknown[] }>(
  lists: readonly T[],
): T[] {
  return lists.filter((list) => list.locations.length > 0);
}
