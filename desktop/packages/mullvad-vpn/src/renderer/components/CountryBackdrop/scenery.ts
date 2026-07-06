import { ConnectionPhase } from '../../lib/connection-phase';

// Runtime path (served statically from the app root, like every other asset).
const SCENERY_BASE = 'assets/images/scenery';

// Only these exits have dedicated cityscape art. Every other country (and the
// disconnected "exposed" state) falls back to the generic plain. Keys are the
// normalized (lower-case, trimmed) advertised country name.
const COUNTRY_IMAGE: Readonly<Record<string, string>> = {
  germany: 'germany.webp',
  netherlands: 'netherlands.webp',
  singapore: 'singapore.webp',
};

export const PLAINE_IMAGE = `${SCENERY_BASE}/plaine.webp`;
export const TERRIER_IMAGE = `${SCENERY_BASE}/terrier.webp`;
export const BULA_IMAGE = `${SCENERY_BASE}/bula.webp`;

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
// protected, the exit country. Disconnected is always the plain so the rabbit
// reads as exposed in neutral open country, never "already somewhere".
export function resolveScenery(phase: ConnectionPhase, exitCountry: string | undefined): Scenery {
  switch (phase) {
    case 'exposed':
      return { image: PLAINE_IMAGE, showBula: true, blurred: false };
    case 'connecting':
      // Background swaps to the target country and blurs; the rabbit is left
      // untouched (still outside) until the tunnel is actually up.
      return { image: resolveCountryImage(exitCountry), showBula: true, blurred: true };
    case 'protected':
      return { image: resolveCountryImage(exitCountry), showBula: false, blurred: false };
    case 'blocked':
      // Kill switch: nothing leaks, so the rabbit is tucked in the burrow, but
      // there is no tunnel, so the scene stays the neutral plain (no city).
      return { image: PLAINE_IMAGE, showBula: false, blurred: false };
  }
}
