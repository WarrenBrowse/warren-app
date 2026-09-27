import { sprintf } from 'sprintf-js';

import { messages } from '../../../../../../shared/gettext';
import type { GeographicalLocation } from '../../../../../features/locations/types';
import { useAccordionContext } from '../../../../../lib/components/accordion/AccordionContext';
import { Location } from '../location-list-item';
import { LocationListsAction } from './LocationListsAction';
import type { ListsMenuState } from './use-lists-menu';

export type LocationRowActionsProps = {
  location: GeographicalLocation;
  menu?: ListsMenuState;
  // The row opens onto several exits.
  expandable: boolean;
};

/** The end of a place's row: its quiet lists button, then a chevron if it opens. */
export function LocationRowActions({ location, menu, expandable }: LocationRowActionsProps) {
  const { expanded } = useAccordionContext();
  if (menu === undefined && !expandable) {
    return null;
  }
  return (
    <Location.Accordion.Header.TrailingActions>
      {menu && <LocationListsAction location={location} menu={menu} />}
      {expandable && (
        <Location.Accordion.Header.AccordionTrigger
          aria-label={sprintf(
            expanded
              ? messages.pgettext('accessibility', 'Collapse %(location)s')
              : messages.pgettext('accessibility', 'Expand %(location)s'),
            { location: location.label },
          )}>
          <Location.Accordion.Header.TrailingActions.Action>
            <Location.Accordion.Header.TrailingActions.Action.Chevron />
          </Location.Accordion.Header.TrailingActions.Action>
        </Location.Accordion.Header.AccordionTrigger>
      )}
    </Location.Accordion.Header.TrailingActions>
  );
}
