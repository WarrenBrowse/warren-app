import { Ownership } from '../../../../shared/daemon-rpc-types';
import type { LocationType } from '../types';
import { useIsDaitaFilterActive } from './use-is-daita-filter-active';
import { useOwnership } from './use-ownership';
import { useProviders } from './use-providers';

export function useActiveFilters(locationType: LocationType) {
  const { activeOwnership } = useOwnership();
  const { providers, activeProviders } = useProviders();

  const isDaitaFilterActive = useIsDaitaFilterActive(locationType);

  const isOwnershipFilterActive = activeOwnership !== Ownership.any;
  const isProvidersFilterActive = activeProviders.length !== providers.length;
  const isAnyFilterActive =
    isOwnershipFilterActive || isProvidersFilterActive || isDaitaFilterActive;

  return {
    isAnyFilterActive,
    isOwnershipFilterActive,
    isProvidersFilterActive,
    isDaitaFilterActive,
  };
}
