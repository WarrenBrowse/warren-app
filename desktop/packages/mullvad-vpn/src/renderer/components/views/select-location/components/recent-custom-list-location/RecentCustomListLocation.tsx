import { useCallback } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../../shared/gettext';
import type { CustomListLocation } from '../../../../../features/locations/types';
import { spacings } from '../../../../../lib/foundations';
import { Location } from '../location-list-item';
import { useLocationListsContext } from '../location-lists/LocationListsContext';
import { listSizeLabel, LocationRowLead } from '../location-row';
import { RecentCustomListTrailingActions } from './components';
import { RecentCustomListProvider } from './RecentCustomListLocationContext';

export type RecentCustomListLocationProps = {
  customList: CustomListLocation;
  disabled?: boolean;
};

const StyledLocationContainer = styled.div`
  margin-bottom: ${spacings.tiny};
`;

function RecentCustomListLocationImpl({
  customList,
  disabled: disabledProp,
}: RecentCustomListLocationProps) {
  const { handleSelect } = useLocationListsContext();

  const disabled = customList.disabled || disabledProp;

  const handleClick = useCallback(() => {
    void handleSelect(customList);
  }, [customList, handleSelect]);

  return (
    <StyledLocationContainer>
      <Location root selected={customList.selected}>
        <Location.ListItem disabled={disabled} level={0}>
          <Location.ListItem.Trigger
            onClick={handleClick}
            aria-label={sprintf(
              // TRANSLATORS: Accessibility label for a button that connects to a location.
              // TRANSLATORS: Available placeholders:
              // TRANSLATORS: %(location)s - The name of the location that will be connected to when the button is clicked.
              messages.pgettext('accessibility', 'Connect to %(location)s'),
              {
                location: customList.label,
              },
            )}>
            <Location.ListItem.Item style={{ minWidth: 0 }}>
              <LocationRowLead
                label={customList.label}
                subtitle={listSizeLabel(customList.locations.length)}
                list
              />
            </Location.ListItem.Item>
          </Location.ListItem.Trigger>
          <RecentCustomListTrailingActions customList={customList} />
        </Location.ListItem>
      </Location>
    </StyledLocationContainer>
  );
}

export function RecentCustomListLocation({ ...props }: RecentCustomListLocationProps) {
  return (
    <RecentCustomListProvider>
      <RecentCustomListLocationImpl {...props} />
    </RecentCustomListProvider>
  );
}
