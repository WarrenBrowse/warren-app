import styled from 'styled-components';

import { FeatureIndicator } from '../../../../../../../lib/components';
import { useSelector } from '../../../../../../../redux/store';
import { useGetFeatureIndicator } from './hooks';

// The chips join the home's chip row as siblings of the other indicators, so
// they wrap together.
const StyledStack = styled.div({
  display: 'contents',
});

export function FeatureIndicators() {
  const tunnelState = useSelector((state) => state.connection.status);
  const featureMap = useGetFeatureIndicator();

  const visible = tunnelState.state === 'connected' || tunnelState.state === 'connecting';
  const indicators = visible ? (tunnelState.featureIndicators ?? []) : [];

  if (indicators.length === 0) {
    return null;
  }

  const sortedIndicators = [...indicators].sort((a, b) => a - b);

  return (
    <StyledStack>
      {sortedIndicators.map((indicator) => {
        const feature = featureMap[indicator];
        return (
          <FeatureIndicator
            key={indicator.toString()}
            data-testid="feature-indicator"
            onClick={feature.onClick}>
            <FeatureIndicator.Text>{feature.label}</FeatureIndicator.Text>
          </FeatureIndicator>
        );
      })}
    </StyledStack>
  );
}
