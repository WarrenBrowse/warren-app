import React from 'react';
import { sprintf } from 'sprintf-js';

import { type ICustomList } from '../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../shared/gettext';
import { Menu } from '../../../../../../lib/components/menu';
import type { MenuOptionProps } from '../../../../../../lib/components/menu-option';
import {
  useAddLocationToCustomList,
  useRemoveLocationFromCustomList,
} from '../../../../../custom-lists/hooks';
import { listHolds, withoutCustomList } from '../../../../../custom-lists/utils';
import type { GeographicalLocation } from '../../../../types';

export type AddLocationToCustomListMenuOptionProps = MenuOptionProps & {
  location: GeographicalLocation;
  customList: ICustomList;
};

/**
 * One list in the lists menu: checked while it holds the place, and a click
 * adds the place or takes it out. The menu stays open, so several lists can be
 * set in a row.
 */
export function AddLocationToCustomListMenuOption({
  location,
  customList,
  ...props
}: AddLocationToCustomListMenuOptionProps) {
  const addLocationToCustomList = useAddLocationToCustomList();
  const removeLocationFromCustomList = useRemoveLocationFromCustomList();
  const place = withoutCustomList(location.details);
  const holds = listHolds(customList, place);

  const handleClick = React.useCallback(async () => {
    if (holds) {
      await removeLocationFromCustomList(customList.id, place);
    } else {
      await addLocationToCustomList(customList.id, place);
    }
  }, [addLocationToCustomList, customList.id, holds, place, removeLocationFromCustomList]);

  return (
    <Menu.Option {...props}>
      <Menu.Option.Trigger
        onClick={handleClick}
        aria-pressed={holds}
        aria-label={sprintf(
          holds
            ? // TRANSLATORS: Accessibility label for a menu option that takes a location out of a custom list.
              // TRANSLATORS: Available placeholders:
              // TRANSLATORS: %(location)s - The name of the location being removed from the list.
              // TRANSLATORS: %(listName)s - The name of the custom list.
              messages.pgettext('accessibility', 'Remove %(location)s from %(listName)s')
            : // TRANSLATORS: This is an accessibility label for a button that adds a location to a custom list.
              // TRANSLATORS: Available placeholders:
              // TRANSLATORS: %(location)s - The name of the location being added to the list.
              // TRANSLATORS: %(listName)s - The name of the custom list the location will be added to.
              messages.pgettext('accessibility', 'Add %(location)s to %(listName)s'),
          { location: location.label, listName: customList.name },
        )}>
        <Menu.Option.Item>
          <Menu.Option.Item.Icon
            icon="checkmark"
            color="green"
            style={{ visibility: holds ? 'visible' : 'hidden' }}
          />
          <Menu.Option.Item.Label>{customList.name}</Menu.Option.Item.Label>
        </Menu.Option.Item>
      </Menu.Option.Trigger>
    </Menu.Option>
  );
}
