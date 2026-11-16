import { useCallback, useState } from 'react';
import styled from 'styled-components';

import { homeTokens } from '../../../../lib/foundations';

const size = homeTokens.controls.thumb;

const StyledFlag = styled.img`
  flex-shrink: 0;
  width: ${size}px;
  height: ${size}px;
  border-radius: 50%;
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
    <StyledFlag
      src={`assets/images/flags/${code.toLowerCase()}.svg`}
      alt=""
      aria-hidden
      onError={onError}
    />
  );
}
