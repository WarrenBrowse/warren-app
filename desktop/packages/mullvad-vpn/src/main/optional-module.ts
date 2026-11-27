import log from '../shared/logging';

// Loads a module the app can run without, typically one backed by a native
// binary the platform loader may refuse. A refusal costs that feature and is
// logged; it never rejects into startup, which once ran ahead of the tray and
// the daemon connection and left nothing to click.
export async function loadOptionalModule<T>(
  feature: string,
  load: () => Promise<T>,
): Promise<T | undefined> {
  try {
    return await load();
  } catch (e) {
    log.error(`${feature} unavailable: ${(e as Error).message}`);
    return undefined;
  }
}
