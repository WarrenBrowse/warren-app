import styled from 'styled-components';

import { FeatureIndicator } from '../../../../../../../lib/components';
import { useSelector } from '../../../../../../../redux/store';
import { useGetFeatureIndicator } from './hooks';

// Active features are shown as a vertical stack of pills floating just above the
// connection card (per the art direction), left-aligned, always visible while the
// tunnel is up or coming up. The tight gap keeps the stack compact.
const StyledStack = styled.div({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-start',
  gap: '5px',
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
