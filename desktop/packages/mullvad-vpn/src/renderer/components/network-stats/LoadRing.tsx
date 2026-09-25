import React from 'react';
import styled from 'styled-components';

import { LoadLevel } from '../../../shared/network-stats';
import { Colors, colors, loadLevelColors, loadRingTrackColor } from '../../lib/foundations';
import { arcDashOffset, ringGeometry } from '../../lib/network-stats';

const SIZES = {
  // The hostname line of the connection card: must fit its text line.
  tiny: { diameter: 14, stroke: 2.5, bandStroke: 1.5 },
  // Location list rows.
  small: { diameter: 22, stroke: 3, bandStroke: 2 },
  // Exit cards and the fleet header.
  large: { diameter: 104, stroke: 8, bandStroke: 3 },
} as const;

export type LoadRingSize = keyof typeof SIZES;

// Tint of the disc a band-only ring encloses: a hint of the band's colour, so
// the closed ring reads as a state rather than as a full gauge.
const BAND_FILL_OPACITY = 0.07;

const StyledRing = styled.div<{ $diameter: number }>(({ $diameter }) => ({
  position: 'relative',
  flexShrink: 0,
  width: `${$diameter}px`,
  height: `${$diameter}px`,
}));

const StyledSvg = styled.svg({
  display: 'block',
});

const StyledArc = styled.circle({
  transition: 'stroke-dashoffset 800ms ease-out, stroke 300ms ease-out',
  '@media (prefers-reduced-motion: reduce)': {
    transition: 'none',
  },
});

const StyledCenter = styled.div({
  position: 'absolute',
  inset: 0,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  textAlign: 'center',
  padding: '12px',
});

export type LoadRingProps = {
  size: LoadRingSize;
  level: LoadLevel;
  // Absent when the exit is below the live threshold: the ring then shows the
  // band alone, a full thin circle with no arc.
  percent?: number;
  // Offline or stale: drawn in the neutral grey whatever the band.
  muted?: boolean;
  // A load that has no band (the fleet's) is drawn in this colour instead.
  color?: Colors;
  children?: React.ReactNode;
} & Pick<React.AriaAttributes, 'aria-label' | 'aria-hidden'>;

export function LoadRing({
  size,
  level,
  percent,
  muted,
  color: colorOverride,
  children,
  ...aria
}: LoadRingProps) {
  const { diameter, stroke, bandStroke } = SIZES[size];
  const color = colors[muted ? 'whiteOnDarkBlue40' : (colorOverride ?? loadLevelColors[level])];
  const bandOnly = percent === undefined;
  const { center, radius, circumference } = ringGeometry(diameter, bandOnly ? bandStroke : stroke);

  return (
    <StyledRing $diameter={diameter} role={aria['aria-label'] ? 'img' : undefined} {...aria}>
      <StyledSvg width={diameter} height={diameter} viewBox={`0 0 ${diameter} ${diameter}`}>
        {bandOnly ? (
          <circle
            data-testid="load-ring-band"
            cx={center}
            cy={center}
            r={radius}
            strokeWidth={bandStroke}
            style={{ stroke: color, fill: color, fillOpacity: BAND_FILL_OPACITY }}
          />
        ) : (
          <>
            <circle
              cx={center}
              cy={center}
              r={radius}
              fill="none"
              strokeWidth={stroke}
              style={{ stroke: colors[loadRingTrackColor] }}
            />
            {percent > 0 && (
              <StyledArc
                data-testid="load-ring-arc"
                cx={center}
                cy={center}
                r={radius}
                fill="none"
                strokeWidth={stroke}
                strokeLinecap="round"
                strokeDasharray={circumference}
                strokeDashoffset={arcDashOffset(percent, circumference)}
                transform={`rotate(-90 ${center} ${center})`}
                style={{ stroke: color }}
              />
            )}
          </>
        )}
      </StyledSvg>
      {children !== undefined && <StyledCenter>{children}</StyledCenter>}
    </StyledRing>
  );
}
