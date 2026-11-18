import { useSelector } from '../../../redux/store';
import { useDaitaDirectOnly, useDaitaEnabled } from '../../daita/hooks';
import { useMultihop } from '../../multihop/hooks';
import { type LocationType } from '../types';
import { filterLocations } from '../utils';
import { useOwnership, useProviders } from '.';

export function useFilterCountryLocations(locationType: LocationType) {
  const locations = useSelector((state) => state.settings.relayLocations);
  const { activeOwnership } = useOwnership();
  const { activeProviders } = useProviders();
  const { daitaEnabled } = useDaitaEnabled();
  const { daitaDirectOnly } = useDaitaDirectOnly();
  const { multihop } = useMultihop();

  return filterLocations({
    locations,
    ownership: activeOwnership,
    providers: activeProviders,
    daita: daitaEnabled,
    directOnly: daitaDirectOnly,
    locationType,
    multihop,
  });
}
