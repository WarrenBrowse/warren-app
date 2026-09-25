import sceneryManifest from '../../../../assets/images/scenery/scenery.json';
import { ConnectionPhase } from '../../lib/connection-phase';

// Runtime path (served statically from the app root, like every other asset).
const SCENERY_BASE = 'assets/images/scenery';

// scenery.json is the one table of layers and phases, shared with the browser
// extension (its design sync copies it) and process-scenery.sh. Only the
// countries it lists have dedicated art; every other one falls back to the
// plain. Keys here are the normalized (lower-case, trimmed) English relay-list
// country name, which is what the daemon reports.
const COUNTRY_IMAGE: Readonly<Record<string, string>> = Object.fromEntries(
  sceneryManifest.layers
    .filter((layer) => layer.role === 'country')
    .map((layer) => [(layer.countryName ?? '').toLowerCase(), `${layer.slug}.webp`]),
);

const layerFile = (role: string) => {
  const layer = sceneryManifest.layers.find((l) => l.role === role);
  if (!layer) throw new Error(`scenery.json has no ${role} layer`);
  return `${SCENERY_BASE}/${layer.slug}.webp`;
};

// The open plain, with the two cameras trained on it: home when no tunnel
// carries the traffic, and the backdrop of any exit with no bespoke art.
export const PLAINE_IMAGE = layerFile('plain');
export const TERRIER_IMAGE = layerFile('burrow');
export const BULA_IMAGE = layerFile('bula');

export interface Scenery {
  // Full asset path of the background landscape.
  image: string;
  // Whether Bula sits exposed on the grass (outside the burrow).
  showBula: boolean;
  // Whether the landscape is blurred (the connecting animation).
  blurred: boolean;
}

export function resolveCountryImage(country: string | undefined): string {
  const key = (country ?? '').trim().toLowerCase();
  const file = COUNTRY_IMAGE[key];
  return file ? `${SCENERY_BASE}/${file}` : PLAINE_IMAGE;
}

// The scenery is driven purely by the visual phase plus, when connecting or
// protected, the exit country. Without a tunnel the backdrop is the watched
// plain, so an unprotected screen shows what unprotected means, and the country
// art is reserved for the states where traffic really goes there.
export function resolveScenery(phase: ConnectionPhase, exitCountry: string | undefined): Scenery {
  // exposed: the watched plain, Bula outside. connecting: the target country,
  // blurred, Bula still outside until the tunnel is up. protected: the country,
  // sharp, Bula in the burrow. interrupted: the country blurred ("not
  // settled"), Bula tucked in because the kill switch holds. blocked: the plain
  // seen only through the blur, Bula tucked in.
  const table = sceneryManifest.phases[phase];
  return {
    image: table.landscape === 'country' ? resolveCountryImage(exitCountry) : PLAINE_IMAGE,
    showBula: table.bula,
    blurred: table.blurred,
  };
}
