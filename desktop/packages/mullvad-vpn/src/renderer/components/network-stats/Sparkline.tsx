import styled from 'styled-components';

import { Colors, colors } from '../../lib/foundations';
import { sparklinePaths } from '../../lib/network-stats';

const StyledSvg = styled.svg({
  display: 'block',
  width: '100%',
  height: 'auto',
  overflow: 'visible',
});

export type SparklineProps = {
  values: readonly number[];
  // Drawing box; the chart scales uniformly to the width of its container.
  width: number;
  height: number;
  color?: Colors;
};

export function Sparkline({ values, width, height, color = 'whiteOnDarkBlue80' }: SparklineProps) {
  const paths = sparklinePaths(values, width, height);
  if (!paths) {
    return null;
  }
  const stroke = colors[color];
  return (
    <StyledSvg viewBox={`0 0 ${width} ${height}`} aria-hidden data-testid="sparkline">
      {paths.area && <path d={paths.area} style={{ fill: stroke, fillOpacity: 0.12 }} />}
      {paths.line && (
        <path
          d={paths.line}
          fill="none"
          strokeWidth={1.5}
          strokeLinejoin="round"
          strokeLinecap="round"
          style={{ stroke }}
        />
      )}
      <circle cx={paths.last.x} cy={paths.last.y} r={2.5} style={{ fill: stroke }} />
    </StyledSvg>
  );
}
