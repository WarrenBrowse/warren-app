import React from 'react';
import { sprintf } from 'sprintf-js';

import { messages } from '../../../../../../../../shared/gettext';
import { useCustomLists } from '../../../../../../../features/custom-lists/hooks';
import {
  GeographicalLocationMenu,
  GeographicalLocationMenuButton,
} from '../../../../../../../features/locations/components';
import { type GeographicalLocation } from '../../../../../../../features/locations/types';
import { useAccordionContext } from '../../../../../../../lib/components/accordion/AccordionContext';
import { holdsSeveralExits } from '../../../../../../../lib/network-stats';
import { Location } from '../../../location-list-item';

export type GeographicalLocationTrailingActionsProps = React.PropsWithChildren<{
  location: GeographicalLocation;
}>;

export function GeographicalLocationTrailingActions({
  location,
}: GeographicalLocationTrailingActionsProps) {
  const { expanded } = useAccordionContext();

  const geographicalLocationButtonRef = React.useRef<HTMLButtonElement>(null);
  const [geographicalLocationMenuOpen, setGeographicalLocationMenuOpen] = React.useState(false);
  const toggleGeographicalLocationMenu = React.useCallback(() => {
    setGeographicalLocationMenuOpen((prev) => !prev);
  }, []);

  const showAccordionTrigger = holdsSeveralExits(location);
  // The menu only adds the row to a custom list; the header menu creates the first one.
  const { customLists } = useCustomLists();
  const showMenu = customLists.length > 0;

  if (!showMenu && !showAccordionTrigger) {
    return null;
  }

  return (
    <Location.Accordion.Header.TrailingActions>
      {showMenu && (
        <Location.Accordion.Header.TrailingActions.Action>
          <GeographicalLocationMenuButton
            ref={geographicalLocationButtonRef}
            location={location}
            onClick={toggleGeographicalLocationMenu}
          />
          <GeographicalLocationMenu
            triggerRef={geographicalLocationButtonRef}
            open={geographicalLocationMenuOpen}
            onOpenChange={setGeographicalLocationMenuOpen}
            location={location}
          />
        </Location.Accordion.Header.TrailingActions.Action>
      )}
      {showAccordionTrigger && (
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
