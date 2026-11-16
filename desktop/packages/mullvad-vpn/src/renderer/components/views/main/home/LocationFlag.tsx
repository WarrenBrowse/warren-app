import { useCallback, useState } from 'react';
import styled from 'styled-components';

import { homeTokens, surfaces } from '../../../../lib/foundations';

const { controls } = homeTokens;

// A hairline over the disc, so a flag with a white field keeps its edge on
// the light theme's paper.
const StyledFrame = styled.span`
  position: relative;
  flex-shrink: 0;
  width: ${controls.thumb}px;
  height: ${controls.thumb}px;
  border-radius: 50%;
  overflow: hidden;
  &::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: 50%;
    box-shadow: inset 0 0 0 ${controls.thumbLine}px ${surfaces.homeFlagLine};
  }
`;

const StyledFlag = styled.img`
  display: block;
  width: 100%;
  height: 100%;
  user-select: none;
  -webkit-user-drag: none;
`;

// The round flag of the location's country, as the mockup's location row
// shows it. A pick that names no country (automatic, a custom list) shows
// only its name.
export function LocationFlag({ code }: { code?: string }) {
  const [failed, setFailed] = useState<string>();
  const onError = useCallback(() => setFailed(code), [code]);
  if (!code || failed === code) {
    return null;
  }
  return (
    <StyledFrame aria-hidden>
      <StyledFlag src={`assets/images/flags/${code.toLowerCase()}.svg`} alt="" onError={onError} />
    </StyledFrame>
  );
}
