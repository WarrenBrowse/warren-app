import { sprintf } from 'sprintf-js';

import { messages } from '../../../shared/gettext';
import type { LoadLevel } from '../../../shared/network-stats';
import type { GeographicalLocation } from '../../features/locations/types';
import type { IRelayLocationCountryRedux } from '../../redux/settings/reducers';

export interface RingGeometry {
  center: number;
  radius: number;
  circumference: number;
}

/** A ring of `diameter` px whose `stroke` stays inside it. */
export function ringGeometry(diameter: number, stroke: number): RingGeometry {
  const radius = (diameter - stroke) / 2;
  return { center: diameter / 2, radius, circumference: 2 * Math.PI * radius };
}

/** The `stroke-dashoffset` that leaves `percent` of the circumference drawn. */
export function arcDashOffset(percent: number, circumference: number): number {
  const share = Math.min(100, Math.max(0, percent)) / 100;
  return circumference * (1 - share);
}

export interface SparklinePaths {
  line: string;
  area: string;
  last: { x: number; y: number };
  inset: number;
}

// Room kept around the curve so the stroke and the last-point dot are not
// clipped by the edge of the box.
const SPARKLINE_INSET = 2;

const round = (value: number) => Math.round(value * 100) / 100;

/**
 * An area-and-line sparkline scaled to its own range: no axes, one scale per
 * chart, oldest point on the left, newest on the right edge.
 */
export function sparklinePaths(
  values: readonly number[],
  width: number,
  height: number,
): SparklinePaths | undefined {
  if (values.length === 0) {
    return undefined;
  }
  const inset = SPARKLINE_INSET;
  const right = width - inset;
  const bottom = height - inset;
  if (values.length === 1) {
    return { line: '', area: '', last: { x: right, y: round(height / 2) }, inset };
  }
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min;
  const points = values.map((value, index) => ({
    x: round(inset + ((right - inset) * index) / (values.length - 1)),
    // A flat series sits on the middle line rather than dividing by zero.
    y: round(span === 0 ? height / 2 : bottom - ((bottom - inset) * (value - min)) / span),
  }));
  const line = points.map(({ x, y }, index) => `${index === 0 ? 'M' : 'L'}${x},${y}`).join(' ');
  const first = points[0];
  const last = points[points.length - 1];
  const area = `${line} L${last.x},${height} L${first.x},${height} Z`;
  return { line, area, last, inset };
}

/** Ease-out cubic between two snapshots. Never extrapolates past `to`. */
export function tweenAt(from: number, to: number, elapsedMs: number, durationMs: number): number {
  const progress = Math.min(1, Math.max(0, elapsedMs / durationMs));
  const eased = 1 - (1 - progress) ** 3;
  return from + (to - from) * eased;
}

function relayHostnames(location: GeographicalLocation): string[] {
  switch (location.type) {
    case 'relay':
      return [location.details.hostname];
    case 'city':
      return location.relays.map((relay) => relay.details.hostname);
    case 'country':
      return location.cities.flatMap((city) => city.relays.map((relay) => relay.details.hostname));
  }
}

/**
 * The exit a row stands for: its relay, or the single exit of a city or a
 * country. A place with several exits has no one load to show.
 */
export function singleExitHostname(location: GeographicalLocation): string | undefined {
  const hostnames = relayHostnames(location);
  return hostnames.length === 1 ? hostnames[0] : undefined;
}

export interface RelayPlace {
  country: string;
  city: string;
}

/** Where each exit is, as the signed relay list says (English names). */
export function relayPlacesByHostname(
  countries: readonly IRelayLocationCountryRedux[],
): Map<string, RelayPlace> {
  const places = new Map<string, RelayPlace>();
  for (const country of countries) {
    for (const city of country.cities) {
      for (const relay of city.relays) {
        places.set(relay.hostname, { country: country.name, city: city.name });
      }
    }
  }
  return places;
}

export function loadLevelLabel(level: LoadLevel): string {
  switch (level) {
    case 'low':
      // TRANSLATORS: Load band of a Warren exit: plenty of room.
      return messages.pgettext('network-stats', 'Low load');
    case 'moderate':
      // TRANSLATORS: Load band of a Warren exit: busy, still comfortable.
      return messages.pgettext('network-stats', 'Moderate load');
    case 'high':
      // TRANSLATORS: Load band of a Warren exit: close to its limit.
      return messages.pgettext('network-stats', 'High load');
    case 'saturated':
      // TRANSLATORS: Load band of a Warren exit: at its limit.
      return messages.pgettext('network-stats', 'Saturated');
    case 'unknown':
      // TRANSLATORS: Shown when the load of a Warren exit is not known.
      return messages.pgettext('network-stats', 'Load unknown');
  }
}

/** "Updated N s ago", in the largest whole unit. */
export function formatSnapshotAge(ageSecs: number): string {
  if (ageSecs < 60) {
    return sprintf(
      // TRANSLATORS: Age of the network figures on screen.
      // TRANSLATORS: Available placeholders:
      // TRANSLATORS: %(seconds)d - whole seconds since the figures were measured
      messages.pgettext('network-stats', 'Updated %(seconds)d s ago'),
      { seconds: ageSecs },
    );
  }
  if (ageSecs < 3600) {
    return sprintf(
      // TRANSLATORS: Age of the network figures on screen.
      // TRANSLATORS: Available placeholders:
      // TRANSLATORS: %(minutes)d - whole minutes since the figures were measured
      messages.pgettext('network-stats', 'Updated %(minutes)d min ago'),
      { minutes: Math.floor(ageSecs / 60) },
    );
  }
  return sprintf(
    // TRANSLATORS: Age of the network figures on screen.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(hours)d - whole hours since the figures were measured
    messages.pgettext('network-stats', 'Updated %(hours)d h ago'),
    { hours: Math.floor(ageSecs / 3600) },
  );
}

/** Why a quiet exit shows its load band and nothing else. */
export function liveThresholdNote(threshold: number): string {
  return sprintf(
    // TRANSLATORS: Why a quiet Warren exit shows its load band and nothing else.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(threshold)d - fewest people an exit must carry for its live figures to show
    messages.pgettext(
      'network-stats',
      'Live figures appear once %(threshold)d people share an exit, so no one’s traffic can be singled out.',
    ),
    { threshold },
  );
}
