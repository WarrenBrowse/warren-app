import { sprintf } from 'sprintf-js';

import { messages } from '../../../shared/gettext';
import type { LoadLevel } from '../../../shared/network-stats';
import type { GeographicalLocation } from '../../features/locations/types';

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
 * Whether a row stands for more than one exit, and so opens into a list. A
 * place with one exit is that exit: the row itself selects it.
 */
export function holdsSeveralExits(location: GeographicalLocation): boolean {
  return relayHostnames(location).length > 1;
}

/**
 * Whether a row reads as the current selection. A place with one exit shows no
 * children, so it carries the selection of its exit or of the city holding it.
 */
export function showsAsSelected(location: GeographicalLocation): boolean {
  if (location.selected) {
    return true;
  }
  if (holdsSeveralExits(location)) {
    return false;
  }
  switch (location.type) {
    case 'relay':
      return false;
    case 'city':
      return location.relays.some((relay) => relay.selected);
    case 'country':
      return location.cities.some(showsAsSelected);
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

/**
 * The arc of a quiet exit's ring. It has a band and no percentage, and the arc
 * gives the band a shape as well as a colour.
 */
export function bandArcPercent(level: LoadLevel): number {
  switch (level) {
    case 'low':
      return 25;
    case 'moderate':
      return 60;
    case 'high':
      return 85;
    case 'saturated':
      return 100;
    case 'unknown':
      return 0;
  }
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
