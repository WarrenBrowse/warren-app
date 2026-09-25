import styled from 'styled-components';

import { AppRouteLine } from '../../../../shared/app-routing';
import { tinyText } from '../../../components/common-styles';
import { Colors, colors } from '../../../lib/foundations';
import { appRouteLineText } from '../strings';

function dotColor(line: AppRouteLine): Colors {
  switch (line.kind) {
    case 'connected':
      return 'green';
    case 'connecting':
    case 'waiting':
      return 'yellow';
    case 'unavailable':
      return line.reason === 'tunnel-down' ? 'yellow' : 'red';
    case 'paused':
    case 'bypassed':
      return 'whiteAlpha40';
  }
}

const StyledLine = styled.span({
  ...tinyText,
  fontWeight: 400,
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  // One line whatever the state, so a status update never moves the rows.
  height: '18px',
  minWidth: 0,
  color: colors.whiteAlpha60,
});

const StyledDot = styled.span<{ $color: string }>((props) => ({
  width: '7px',
  height: '7px',
  borderRadius: '50%',
  flexShrink: 0,
  backgroundColor: props.$color,
}));

const StyledText = styled.span({
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
});

export function RouteStatusLine({ line }: { line: AppRouteLine }) {
  const text = appRouteLineText(line);
  return (
    <StyledLine>
      <StyledDot $color={colors[dotColor(line)]} aria-hidden />
      <StyledText title={text}>{text}</StyledText>
    </StyledLine>
  );
}
