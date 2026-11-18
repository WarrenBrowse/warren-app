import { Ownership } from '../../../../shared/daemon-rpc-types';
import type { IRelayLocationCountryRedux } from '../../../redux/settings/reducers';
import type { LocationType } from '../types';
import { filterLocationsByFilters } from './filter-locations-by-filters';
import { getDaitaFilter } from './get-daita-filter';
import { getOwnershipFilter } from './get-ownership-filter';
import { getProviderFilter } from './get-provider-filter';

export function filterLocations({
  locations,
  ownership,
  providers,
  daita,
  directOnly,
  locationType,
  multihop,
}: {
  locations: IRelayLocationCountryRedux[];
  locationType: LocationType;
  ownership?: Ownership;
  providers?: Array<string>;
  daita: boolean;
  directOnly: boolean;
  multihop: boolean;
}): IRelayLocationCountryRedux[] {
  const ownershipFilter = getOwnershipFilter(ownership);
  const providerFilter = getProviderFilter(providers);
  const daitaFilter = getDaitaFilter(daita, directOnly, locationType, multihop);
  const filters = [ownershipFilter, providerFilter, daitaFilter];

  const anyFilterActive = filters.some((filter) => filter !== undefined);

  return anyFilterActive
    ? filterLocationsByFilters(locations, (relay) =>
        filters.every((filter) => filter?.(relay) ?? true),
      )
    : locations;
}
