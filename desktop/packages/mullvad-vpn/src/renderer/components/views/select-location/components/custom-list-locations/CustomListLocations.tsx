import React from 'react';
import styled from 'styled-components';

import { listsWithLocations } from '../../../../../features/custom-lists/utils';
import { AnimatedList } from '../../../../../lib/components/animated-list';
import { FlexColumn } from '../../../../../lib/components/flex-column';
import { useSelectLocationViewContext } from '../../SelectLocationViewContext';
import { getLocationListItemMapProps } from '../../utils';
import { CustomListLocation } from '../custom-list-location';
import { CustomListsSectionTitle } from './components';

const StyledAnimatedList = styled(AnimatedList)`
  display: flex;
  flex-direction: column;
`;

function CustomListLocationsImpl() {
  const { customListLocations } = useSelectLocationViewContext();
  const titleId = React.useId();
  const shownLists = listsWithLocations(customListLocations);

  return (
    <FlexColumn as="section" aria-labelledby={titleId} gap="tiny">
      <CustomListsSectionTitle id={titleId} />
      <FlexColumn>
        <StyledAnimatedList>
          {shownLists.map((customList) => {
            const { key } = getLocationListItemMapProps(customList, undefined);
            return (
              <AnimatedList.Item key={key}>
                <CustomListLocation customList={customList} />
              </AnimatedList.Item>
            );
          })}
        </StyledAnimatedList>
      </FlexColumn>
    </FlexColumn>
  );
}

export function CustomListLocations() {
  return <CustomListLocationsImpl />;
}
