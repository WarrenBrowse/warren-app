import styled, { css, keyframes } from 'styled-components';

import { NetworkStats } from '../../../shared/network-stats';
import { colors } from '../../lib/foundations';
import { formatSnapshotAge, useSnapshotAge } from '../../lib/network-stats';

const pulse = keyframes`
  0% {
    box-shadow: 0 0 0 0 ${colors.greenAlpha40};
  }
  70% {
    box-shadow: 0 0 0 5px transparent;
  }
  100% {
    box-shadow: 0 0 0 0 transparent;
  }
`;

const StyledLive = styled.span({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '6px',
  color: colors.whiteAlpha60,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '11px',
  lineHeight: '16px',
  whiteSpace: 'nowrap',
});

const StyledDot = styled.span<{ $fresh: boolean }>`
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: ${({ $fresh }) => ($fresh ? colors.green : colors.whiteOnDarkBlue40)};
  ${({ $fresh }) =>
    $fresh &&
    css`
      animation: ${pulse} 2s ease-out infinite;
    `}

  @media (prefers-reduced-motion: reduce) {
    animation: none;
  }
`;

export type LiveIndicatorProps = {
  stats: NetworkStats;
  stale: boolean;
};

export function LiveIndicator({ stats, stale }: LiveIndicatorProps) {
  const age = useSnapshotAge(stats) ?? 0;
  return (
    <StyledLive data-testid="network-stats-freshness">
      <StyledDot $fresh={!stale} />
      {formatSnapshotAge(age)}
    </StyledLive>
  );
}
