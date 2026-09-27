import styled from 'styled-components';

import { AnimatedList } from '../../../../../../../lib/components/animated-list';
import { getLocationListItemMapProps } from '../../../../utils';
import { RecentCustomListLocation } from '../../../recent-custom-list-location';
import { RecentGeographicalLocation } from '../../../recent-geographical-location';
import { useRecentLocations } from './hooks';

const StyledAnimatedList = styled(AnimatedList)`
  display: flex;
  flex-direction: column;
`;

export function RecentLocationList() {
  const recentLocations = useRecentLocations();

  return (
    <StyledAnimatedList>
      {recentLocations.map((location) => {
        const { key } = getLocationListItemMapProps(location);
        if (location.type === 'customList') {
          return (
            <AnimatedList.Item key={key}>
              <RecentCustomListLocation customList={location} />
            </AnimatedList.Item>
          );
        } else {
          return (
            <AnimatedList.Item key={key}>
              <RecentGeographicalLocation location={location} />
            </AnimatedList.Item>
          );
        }
      })}
    </StyledAnimatedList>
  );
}
