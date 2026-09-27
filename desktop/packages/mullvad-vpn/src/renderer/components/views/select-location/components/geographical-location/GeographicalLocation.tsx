import { useCallback, useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';

import { messages } from '../../../../../../shared/gettext';
import { useRecents } from '../../../../../features/locations/hooks';
import { type GeographicalLocation } from '../../../../../features/locations/types';
import { getLocationChildren } from '../../../../../features/locations/utils';
import { type ListItemProps } from '../../../../../lib/components/list-item';
import { holdsSeveralExits, showsAsSelected } from '../../../../../lib/network-stats';
import { useScrollPositionContext } from '../../ScrollPositionContext';
import { getLocationListItemMapProps } from '../../utils';
import { Location } from '../location-list-item';
import { LocationRowActions, LocationRowLead, useListsMenu } from '../location-row';
import { LocationExitLoad } from './components';
import {
  GeographicalLocationProvider,
  useGeographicalLocationContext,
} from './GeographicalLocationContext';

export type GeographicalLocationProps = Pick<ListItemProps, 'level' | 'position'> & {
  location: GeographicalLocation;
  root?: boolean;
  disabled?: boolean;
  onSelect: (location: GeographicalLocation) => void;
  expanded?: boolean;
};

function GeographicalLocationImpl({
  location,
  level,
  disabled: disabledProp,
  root,
  position,
  onSelect,
  ...props
}: GeographicalLocationProps) {
  const { loading } = useGeographicalLocationContext();
  const [expanded, setExpanded] = useState(location.expanded);
  const locationChildren = getLocationChildren(location);
  const { selectedLocationRef } = useScrollPositionContext();
  const { hasRecents } = useRecents();

  useEffect(() => {
    setExpanded(location.expanded);
  }, [location.expanded]);

  const menu = useListsMenu();
  const disabled = disabledProp || location.disabled || loading;
  // A place with a single exit is that exit: it never opens.
  const showChildren = holdsSeveralExits(location) && expanded;

  const handleClick = useCallback(() => {
    onSelect(location);
  }, [location, onSelect]);

  const handleSelect = useCallback(
    (location: GeographicalLocation) => {
      onSelect(location);
    },
    [onSelect],
  );

  const renderChildren = () => {
    return locationChildren.map((locationChild) => {
      const { key, nextLevel } = getLocationListItemMapProps(locationChild, level);
      return (
        <GeographicalLocation
          key={key}
          location={locationChild}
          level={nextLevel}
          disabled={disabled}
          onSelect={handleSelect}
          {...props}
        />
      );
    });
  };

  // Only scroll to the selected location when the recents feature is disabled
  const shouldScrollToLocation = location.selected && !hasRecents;
  const refToScrollTo = shouldScrollToLocation ? selectedLocationRef : null;

  return (
    <Location selected={showsAsSelected(location)} root={root}>
      <Location.Accordion expanded={expanded} onExpandedChange={setExpanded} disabled={disabled}>
        <Location.Accordion.Header ref={refToScrollTo} level={level} position={position}>
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
                country={location.type === 'country' ? location.details.country : undefined}
              />
              {(location.type === 'relay' || !showChildren) && (
                <LocationExitLoad location={location} />
              )}
            </Location.Accordion.Header.Item>
          </Location.Accordion.Header.ItemTrigger>
          <LocationRowActions
            location={location}
            menu={menu}
            expandable={holdsSeveralExits(location)}
          />
        </Location.Accordion.Header>
        <Location.Accordion.Content>
          {showChildren ? renderChildren() : null}
        </Location.Accordion.Content>
      </Location.Accordion>
    </Location>
  );
}

export function GeographicalLocation({ ...props }: GeographicalLocationProps) {
  return (
    <GeographicalLocationProvider>
      <GeographicalLocationImpl {...props} />
    </GeographicalLocationProvider>
  );
}
