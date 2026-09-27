import { useCallback } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../../shared/gettext';
import type { GeographicalLocation } from '../../../../../features/locations/types';
import { spacings } from '../../../../../lib/foundations';
import { joinList } from '../../../../../lib/list-format';
import { LocationExitLoad } from '../geographical-location/components';
import { Location } from '../location-list-item';
import { useLocationListsContext } from '../location-lists/LocationListsContext';
import { LocationRowActions, LocationRowLead, useListsMenu } from '../location-row';
import { useLocationBreadcrumbs } from './hooks';
import { RecentGeographicalLocationProvider } from './RecentGeographicalLocationContext';

export type RecentGeographicalLocationProps = {
  location: GeographicalLocation;
  disabled?: boolean;
};

const StyledLocationContainer = styled.div`
  margin-bottom: ${spacings.tiny};
`;

function RecentGeographicalLocationImpl({
  location,
  disabled: disabledProp,
}: RecentGeographicalLocationProps) {
  const { handleSelect } = useLocationListsContext();

  const locationBreadcrumbs = useLocationBreadcrumbs(location);
  const breadcrumbsSubLabel = joinList(locationBreadcrumbs);

  const disabled = location.disabled || disabledProp;

  const showParents = location.type !== 'country';
  const menu = useListsMenu();

  const handleClick = useCallback(() => {
    void handleSelect(location);
  }, [location, handleSelect]);

  return (
    <StyledLocationContainer>
      <Location root selected={location.selected}>
        <Location.Accordion expanded disabled={disabled}>
          <Location.Accordion.Header level={0}>
            <Location.Accordion.Header.ItemTrigger
              style={{ minWidth: 0 }}
              onClick={handleClick}
              onContextMenu={menu.onContextMenu}
              aria-label={sprintf(
                // TRANSLATORS: Accessibility label for a button that connects to a location.
                // TRANSLATORS: Available placeholders:
                // TRANSLATORS: %(location)s - The name of the location that will be connected to when the button is clicked.
                messages.pgettext('accessibility', 'Connect to %(location)s'),
                {
                  location: location.label,
                },
              )}>
              <Location.Accordion.Header.Item style={{ minWidth: 0 }}>
                <LocationRowLead
                  label={location.label}
                  subtitle={showParents ? breadcrumbsSubLabel : undefined}
                  country={location.details.country}
                />
                <LocationExitLoad location={location} />
              </Location.Accordion.Header.Item>
            </Location.Accordion.Header.ItemTrigger>
            <LocationRowActions location={location} menu={menu} expandable={false} />
          </Location.Accordion.Header>
        </Location.Accordion>
      </Location>
    </StyledLocationContainer>
  );
}

export function RecentGeographicalLocation({ ...props }: RecentGeographicalLocationProps) {
  return (
    <RecentGeographicalLocationProvider>
      <RecentGeographicalLocationImpl {...props} />
    </RecentGeographicalLocationProvider>
  );
}
