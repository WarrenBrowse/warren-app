import styled from 'styled-components';

import {
  GeographicalLocationMenu,
  GeographicalLocationMenuButton,
} from '../../../../../features/locations/components';
import type { GeographicalLocation } from '../../../../../features/locations/types';
import { Location } from '../location-list-item';
import type { ListsMenuState } from './use-lists-menu';

// Hidden until the row is pointed at or focused, and while its menu is open;
// the `LocationRevealScope` of the lists holds the rule.
const StyledReveal = styled.span({
  display: 'contents',
});

export type LocationListsActionProps = {
  location: GeographicalLocation;
  menu: ListsMenuState;
};

/** The quiet button at the end of a row that opens its lists menu. */
export function LocationListsAction({ location, menu }: LocationListsActionProps) {
  return (
    <Location.Accordion.Header.TrailingActions.Action data-lists-action>
      <StyledReveal>
        <GeographicalLocationMenuButton
          ref={menu.triggerRef}
          location={location}
          onClick={menu.toggle}
          data-reveal
          data-open={menu.open}
        />
      </StyledReveal>
      <GeographicalLocationMenu
        triggerRef={menu.triggerRef}
        open={menu.open}
        onOpenChange={menu.setOpen}
        location={location}
      />
    </Location.Accordion.Header.TrailingActions.Action>
  );
}
