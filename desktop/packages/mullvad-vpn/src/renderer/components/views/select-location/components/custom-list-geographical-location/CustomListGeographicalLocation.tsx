import { useCallback, useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';

import { messages } from '../../../../../../shared/gettext';
import { type GeographicalLocation } from '../../../../../features/locations/types';
import { getLocationChildren } from '../../../../../features/locations/utils';
import { AnimatedList } from '../../../../../lib/components/animated-list';
import { ListItemProps } from '../../../../../lib/components/list-item';
import { holdsSeveralExits } from '../../../../../lib/network-stats';
import { getLocationListItemMapProps } from '../../utils';
import { LocationExitLoad } from '../geographical-location/components';
import { Location } from '../location-list-item';
import { LocationRowActions, LocationRowLead, useListsMenu } from '../location-row';
import { useLocationBreadcrumbs } from '../recent-geographical-location/hooks';
import {
  CustomListGeographicalLocationProvider,
  useCustomListGeographicalLocationContext,
} from './CustomListGeographicalLocationContext';
import { useHandleSelectLocationInCustomList } from './hooks';
export type CustomListGeographicalLocationProps = Pick<ListItemProps, 'level' | 'position'> & {
  disabled?: boolean;
  location: GeographicalLocation;
};

function CustomListGeographicalLocationImpl({
  disabled: disabledProp,
  position,
}: Omit<CustomListGeographicalLocationProps, 'location' | 'level'>) {
  const { loading, location, level } = useCustomListGeographicalLocationContext();
  const [expanded, setExpanded] = useState(location.expanded);

  const locationChildren = getLocationChildren(location);
  const expandable = holdsSeveralExits(location);
  const showChildren = expandable && expanded;
  // A place the list holds leads with its flag and names where it is; the
  // exits it opens onto sit under it without.
  const listed = level === 1;
  const menu = useListsMenu();
  const breadcrumbs = useLocationBreadcrumbs(location).join(', ');
  const disabled = disabledProp || location.disabled || loading;

  useEffect(() => {
    setExpanded(location.expanded);
  }, [location.expanded]);

  const handleSelectLocationInCustomList = useHandleSelectLocationInCustomList();

  const handleClick = useCallback(() => {
    void handleSelectLocationInCustomList(location);
  }, [location, handleSelectLocationInCustomList]);

  const children = locationChildren.map((locationChild) => {
    const { key, nextLevel } = getLocationListItemMapProps(locationChild, level);
    return (
      <CustomListGeographicalLocation
        key={key}
        location={locationChild}
        level={nextLevel}
        disabled={disabled}
        position={position}
      />
    );
  });

  return (
    <Location selected={location.selected}>
      <Location.Accordion expanded={expanded} onExpandedChange={setExpanded} disabled={disabled}>
        <Location.Accordion.Header level={level} position={position}>
          <Location.Accordion.Header.ItemTrigger
            style={{ minWidth: 0 }}
            onClick={handleClick}
            onContextMenu={listed ? menu.onContextMenu : undefined}
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
                subtitle={listed && breadcrumbs !== '' ? breadcrumbs : undefined}
                country={listed ? location.details.country : undefined}
              />
              {(location.type === 'relay' || !showChildren) && (
                <LocationExitLoad location={location} />
              )}
            </Location.Accordion.Header.Item>
          </Location.Accordion.Header.ItemTrigger>

          <LocationRowActions
            location={location}
            menu={listed ? menu : undefined}
            expandable={expandable}
          />
        </Location.Accordion.Header>
        <Location.Accordion.Content>
          <AnimatedList>{showChildren ? children : null}</AnimatedList>
        </Location.Accordion.Content>
      </Location.Accordion>
    </Location>
  );
}

export function CustomListGeographicalLocation({
  location,
  level,
  ...props
}: CustomListGeographicalLocationProps) {
  return (
    <CustomListGeographicalLocationProvider location={location} level={level}>
      <CustomListGeographicalLocationImpl {...props} />
    </CustomListGeographicalLocationProvider>
  );
}
