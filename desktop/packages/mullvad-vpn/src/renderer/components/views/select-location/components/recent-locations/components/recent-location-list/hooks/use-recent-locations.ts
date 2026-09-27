import { LocationType } from '../../../../../../../../features/locations/types';
import { useMultihop } from '../../../../../../../../features/multihop/hooks';
import { useSelectLocationViewContext } from '../../../../../SelectLocationViewContext';

export function useRecentLocations() {
  return useAllRecentLocations().filter(
    // A list that holds nothing is not shown anywhere, recents included.
    (location) => location.type !== 'customList' || location.locations.length > 0,
  );
}

function useAllRecentLocations() {
  const {
    locationType,
    recentMultihopEntryLocations,
    recentMultihopExitLocations,
    recentSinglehopLocations,
  } = useSelectLocationViewContext();
  const { multihop } = useMultihop();
  if (!multihop) {
    if (recentSinglehopLocations) {
      return recentSinglehopLocations;
    }
  } else {
    if (recentMultihopEntryLocations && locationType === LocationType.entry) {
      return recentMultihopEntryLocations;
    } else if (recentMultihopExitLocations && locationType === LocationType.exit) {
      return recentMultihopExitLocations;
    }
  }
  return [];
}
