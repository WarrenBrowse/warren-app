import styled from 'styled-components';

import { LoadLevel } from '../../../shared/network-stats';
import { colors, loadLevelColors, loadRingTrackColor } from '../../lib/foundations';
import { arcDashOffset, ringGeometry } from '../../lib/network-stats';

// Sized to sit inside a line of small text.
const DIAMETER = 12;
const STROKE = 2;

const StyledSvg = styled.svg({
  display: 'block',
  flexShrink: 0,
});

const StyledArc = styled.circle({
  transition: 'stroke-dashoffset 800ms ease-out, stroke 300ms ease-out',
  '@media (prefers-reduced-motion: reduce)': {
    transition: 'none',
  },
});

export type LoadRingProps = {
  level: LoadLevel;
  // Share of the ring drawn, 0 to 100: the load of a live exit, or the arc
  // standing for the band of a quiet one.
  percent: number;
  // Offline or stale: drawn in the neutral grey whatever the band.
  muted?: boolean;
};

// A surface that paints its own palette (the connection card, which turns cream
// in the light theme) redefines these properties; everywhere else the ring
// keeps the band colours of the palette.
function bandColor(level: LoadLevel): string {
  const fallback = colors[loadLevelColors[level]];
  return level === 'unknown' ? fallback : `var(--load-ring-${level}, ${fallback})`;
}

export function LoadRing({ level, percent, muted }: LoadRingProps) {
  const color = muted ? colors.whiteOnDarkBlue40 : bandColor(level);
  const { center, radius, circumference } = ringGeometry(DIAMETER, STROKE);

  return (
    <StyledSvg
      width={DIAMETER}
      height={DIAMETER}
      viewBox={`0 0 ${DIAMETER} ${DIAMETER}`}
      aria-hidden>
      <circle
        data-testid="load-ring-track"
        cx={center}
        cy={center}
        r={radius}
        fill="none"
        strokeWidth={STROKE}
        style={{ stroke: `var(--load-ring-track, ${colors[loadRingTrackColor]})` }}
      />
      {percent > 0 && (
        <StyledArc
          data-testid="load-ring-arc"
          cx={center}
          cy={center}
          r={radius}
          fill="none"
          strokeWidth={STROKE}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={arcDashOffset(percent, circumference)}
          transform={`rotate(-90 ${center} ${center})`}
          style={{ stroke: color }}
        />
      )}
    </StyledSvg>
  );
}
