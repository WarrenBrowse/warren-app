import { useRecents } from '../../../../../features/locations/hooks';
import type { LocationType } from '../../../../../features/locations/types';
import { Expandable } from '../../../../../lib/components/expandable';
import { FlexColumn } from '../../../../../lib/components/flex-column';
import { useHasCustomLists } from '../../hooks';
import { CountryLocations } from '../country-locations';
import { CustomListLocations } from '../custom-list-locations';
import { NoSearchResult } from '../no-search-result';
import { RecentLocations } from '../recent-locations';
import { useRecentLocations } from '../recent-locations/components/recent-location-list/hooks';
import { useHasSearched, useHasSearchedLocations } from './hooks';
import { LocationListsProvider } from './LocationListsContext';

export type LocationsListsProps = React.PropsWithChildren & {
  type: LocationType;
};

export function LocationLists(props: LocationsListsProps) {
  const { hasRecents } = useRecents();
  const recentLocations = useRecentLocations();
  const hasSearched = useHasSearched();
  const hasVisibleCustomLists = useHasCustomLists();
  const hasSearchedLocations = useHasSearchedLocations();

  // A section with nothing in it is not shown: the header menu creates the
  // first custom list, and recents appear once there is one.
  const showRecentLocations = !hasSearched && hasRecents && recentLocations.length > 0;
  const showCustomListLocationLists = hasVisibleCustomLists;
  const showCountryLocations = !hasSearched || hasSearchedLocations;
  const showNoSearchResult =
    hasSearched && !showCustomListLocationLists && !showCountryLocations && !showRecentLocations;

  return (
    <LocationListsProvider {...props}>
      <Expandable expanded={showRecentLocations}>
        <Expandable.Content>
          <RecentLocations />
        </Expandable.Content>
      </Expandable>
      <FlexColumn gap="large">
        {showCustomListLocationLists && <CustomListLocations />}
        {showCountryLocations && (
          <CountryLocations showTitle={showRecentLocations || showCustomListLocationLists} />
        )}
        {showNoSearchResult && <NoSearchResult />}
      </FlexColumn>
    </LocationListsProvider>
  );
}
