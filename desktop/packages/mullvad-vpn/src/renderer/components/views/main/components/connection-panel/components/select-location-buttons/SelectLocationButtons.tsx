import styled from 'styled-components';

import { SelectLocationButton, ShuffleButton } from './components';

const StyledRow = styled.div({
  display: 'flex',
  gap: '4px',
});

// The location selector always carries a shuffle button (random exit) at its
// side, in every connection state.
export function SelectLocationButtons() {
  return (
    <StyledRow>
      <SelectLocationButton />
      <ShuffleButton />
    </StyledRow>
  );
}
