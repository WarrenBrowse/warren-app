import type { LoadLevel } from '../../../../shared/network-stats';
import type { Colors } from './color-variables';

// The colour of an exit's load band. The band is decided server-side so every
// client paints the same exit the same way; this only maps it onto the palette.
// A band this client does not know, or no band at all, stays neutral.
export const loadLevelColors: Record<LoadLevel, Colors> = {
  low: 'green',
  moderate: 'yellow',
  high: 'orange',
  saturated: 'red',
  unknown: 'whiteOnDarkBlue40',
};

// The unfilled part of a load ring.
export const loadRingTrackColor: Colors = 'whiteAlpha20';
