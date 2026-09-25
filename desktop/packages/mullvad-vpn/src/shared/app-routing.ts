import type {
  AppExit,
  AppRouteStatus,
  AppRouteUnavailableReason,
  AppRoutingSettings,
  AppSplitMode,
  ExitChoice,
} from './daemon-rpc-types';

// Route sessions besides the main connection: the three session tokens of an
// epoch, one of them the main one. Mirrors `MAX_APP_EXITS` in mullvad-types.
export const MAX_APP_EXITS = 2;

type Platform = NodeJS.Platform;

// Windows and macOS paths are case insensitive (the daemon and the app list
// compare them that way); a Linux path names another program when its case
// differs.
export function sameAppId(a: string, b: string, platform: Platform): boolean {
  if (platform === 'linux') {
    return a === b;
  }
  return a.toLowerCase() === b.toLowerCase();
}

function includesApp(apps: readonly string[], app: string, platform: Platform): boolean {
  return apps.some((other) => sameAppId(other, app, platform));
}

export function sameExitChoice(a: ExitChoice, b: ExitChoice): boolean {
  return a.country === b.country && (a.city ?? '') === (b.city ?? '');
}

export function exitChoicesInUse(appExits: readonly AppExit[]): ExitChoice[] {
  const exits: ExitChoice[] = [];
  for (const { exit } of appExits) {
    if (!exits.some((known) => sameExitChoice(known, exit))) {
      exits.push(exit);
    }
  }
  return exits;
}

// Whether the daemon would refuse `exit` for `app` with `app_exit_limit`. It
// counts every saved choice, whatever the switch and the split mode, so the
// GUI does too.
export function wouldExceedAppExitLimit(
  appExits: readonly AppExit[],
  app: string,
  exit: ExitChoice,
  platform: Platform,
): boolean {
  const others = appExits.filter((entry) => !sameAppId(entry.app, app, platform));
  return exitChoicesInUse([...others, { app, exit }]).length > MAX_APP_EXITS;
}

export function appExitFor(
  routing: AppRoutingSettings,
  app: string,
  platform: Platform,
): ExitChoice | undefined {
  return routing.appExits.find((entry) => sameAppId(entry.app, app, platform))?.exit;
}

function isBypassing(routing: AppRoutingSettings, app: string, platform: Platform): boolean {
  return routing.splitMode === 'exclude' && includesApp(routing.excludedApps, app, platform);
}

// The exits in force after the precedence rules of docs/app-routing.md: none
// while the switch is off, and none for an app that bypasses the VPN.
export function effectiveAppExits(routing: AppRoutingSettings, platform: Platform): AppExit[] {
  if (!routing.appExitsEnabled) {
    return [];
  }
  return routing.appExits.filter((entry) => !isBypassing(routing, entry.app, platform));
}

export type AppExitDisplayState = 'none' | 'active' | 'paused' | 'bypassed';

export function appExitDisplayState(
  routing: AppRoutingSettings,
  app: string,
  platform: Platform,
): AppExitDisplayState {
  if (appExitFor(routing, app, platform) === undefined) {
    return 'none';
  }
  if (isBypassing(routing, app, platform)) {
    return 'bypassed';
  }
  return routing.appExitsEnabled ? 'active' : 'paused';
}

// The apps tunneled while include-only is on: the included apps and every app
// with a country in force, since choosing a country puts an app in the VPN.
export function effectiveIncludedApps(routing: AppRoutingSettings, platform: Platform): string[] {
  if (routing.splitMode !== 'include-only') {
    return [];
  }
  const apps = [...routing.includedApps];
  for (const { app } of effectiveAppExits(routing, platform)) {
    if (!includesApp(apps, app, platform)) {
      apps.push(app);
    }
  }
  return apps;
}

export function routeStatusForApp(
  statuses: readonly AppRouteStatus[],
  app: string,
  platform: Platform,
): AppRouteStatus | undefined {
  return statuses.find((status) => includesApp(status.apps, app, platform));
}

export type AppRoutingSummary = {
  // Set only while include-only is on.
  vpnOnlyForCount?: number;
  appsWithOwnCountry: number;
  // A route that cannot run for a reason of its own. A route waiting for the
  // main connection is not a fault.
  anyRouteUnavailable: boolean;
};

export function appRoutingSummary(
  routing: AppRoutingSettings,
  statuses: readonly AppRouteStatus[],
  platform: Platform,
): AppRoutingSummary {
  return {
    vpnOnlyForCount:
      routing.splitMode === 'include-only'
        ? effectiveIncludedApps(routing, platform).length
        : undefined,
    appsWithOwnCountry: effectiveAppExits(routing, platform).length,
    anyRouteUnavailable: statuses.some(
      (status) => status.state === 'unavailable' && status.reason !== 'tunnel-down',
    ),
  };
}

export type ModeChangeConfirmation = {
  leavesDeviceUnprotected: boolean;
  replaces?: Exclude<AppSplitMode, 'off'>;
};

// What the user must accept before the split mode moves from `current` to
// `next`, or undefined when the change needs no confirmation.
export function modeChangeConfirmation(
  current: AppSplitMode,
  next: AppSplitMode,
): ModeChangeConfirmation | undefined {
  if (next === 'off' || next === current) {
    return undefined;
  }
  const replaces = current === 'off' ? undefined : current;
  const leavesDeviceUnprotected = next === 'include-only';
  if (!leavesDeviceUnprotected && replaces === undefined) {
    return undefined;
  }
  return { leavesDeviceUnprotected, replaces };
}

type RelayLocationCountry = {
  name: string;
  code: string;
  cities: ReadonlyArray<{
    name: string;
    code: string;
    relays: ReadonlyArray<{ active: boolean }>;
  }>;
};

export type CountryOption = {
  country: string;
  name: string;
  cities: Array<{ code: string; name: string }>;
};

// The countries and cities a per-app exit can use: those with an active
// server, by translated name. A search matching a country keeps all its
// cities; one matching only cities keeps those.
export function buildCountryOptions(
  locations: readonly RelayLocationCountry[],
  searchTerm: string,
  translate: (name: string) => string,
): CountryOption[] {
  const needle = searchTerm.trim().toLocaleLowerCase();
  const matches = (name: string) => needle === '' || name.toLocaleLowerCase().includes(needle);
  const byName = (a: { name: string }, b: { name: string }) => a.name.localeCompare(b.name);

  const options: CountryOption[] = [];
  for (const location of locations) {
    const cities = location.cities
      .filter((city) => city.relays.some((relay) => relay.active))
      .map((city) => ({ code: city.code, name: translate(city.name) }))
      .sort(byName);
    if (cities.length === 0) {
      continue;
    }
    const name = translate(location.name);
    const shownCities = matches(name) ? cities : cities.filter((city) => matches(city.name));
    if (shownCities.length > 0) {
      options.push({ country: location.code, name, cities: shownCities });
    }
  }
  return options.sort(byName);
}

export type SplitModeAvailability =
  | 'available'
  | 'checking'
  | 'needs-full-disk-access'
  | 'needs-signed-build'
  | 'needs-newer-macos'
  | 'unsupported';

// Whether Bypass VPN and VPN only for can run. Both rest on the same
// classifier, so they share one answer; per-app countries need none of this.
// On macOS the daemon refuses a build it cannot run the classifier on (an
// unsigned one), and the classifier needs Full Disk Access for eslogger.
export function splitModeAvailability(input: {
  platform: Platform;
  supported: boolean;
  needsFullDiskAccess: boolean | undefined;
  isMacOs13OrNewer: boolean;
}): SplitModeAvailability {
  if (input.platform !== 'darwin') {
    return input.supported ? 'available' : 'unsupported';
  }
  if (!input.isMacOs13OrNewer) {
    return 'needs-newer-macos';
  }
  if (!input.supported) {
    return 'needs-signed-build';
  }
  if (input.needsFullDiskAccess === undefined) {
    return 'checking';
  }
  return input.needsFullDiskAccess ? 'needs-full-disk-access' : 'available';
}

export type AppRouteLine =
  | { kind: 'paused' }
  | { kind: 'bypassed' }
  | { kind: 'waiting' }
  | { kind: 'connecting' }
  | { kind: 'connected'; publicIp?: string }
  | { kind: 'unavailable'; reason?: AppRouteUnavailableReason };

// The status line under an app that has a country. A country that is not in
// force says why before any route state, which could be a stale push.
export function appRouteLine(
  routing: AppRoutingSettings,
  statuses: readonly AppRouteStatus[],
  app: string,
  platform: Platform,
): AppRouteLine {
  const displayState = appExitDisplayState(routing, app, platform);
  if (displayState === 'paused' || displayState === 'bypassed') {
    return { kind: displayState };
  }
  const status = routeStatusForApp(statuses, app, platform);
  switch (status?.state) {
    case undefined:
      return { kind: 'waiting' };
    case 'connecting':
      return { kind: 'connecting' };
    case 'connected':
      return { kind: 'connected', publicIp: status.publicIp };
    case 'unavailable':
      return { kind: 'unavailable', reason: status.reason };
  }
}

export function exitChoiceNames(
  exit: ExitChoice,
  locations: readonly RelayLocationCountry[],
  translate: (name: string) => string,
): { country: string; city?: string } {
  const country = locations.find((location) => location.code === exit.country);
  const countryName = country ? translate(country.name) : exit.country.toUpperCase();
  if (exit.city === undefined) {
    return { country: countryName };
  }
  const city = country?.cities.find((candidate) => candidate.code === exit.city);
  return { country: countryName, city: city ? translate(city.name) : exit.city.toUpperCase() };
}

type RoutedApplication = { absolutepath: string; name: string; icon?: string; deletable: boolean };

// The apps behind a list of ids: named by what the main process read for the
// id, else by the app list, else after the file, since an id can come from the
// CLI or a list scanned on another occasion.
export function resolveApplications<T extends RoutedApplication>(
  ids: readonly string[],
  metadata: readonly T[],
  catalog: readonly T[],
  platform: Platform,
): Array<T | RoutedApplication> {
  const find = (list: readonly T[], id: string) =>
    list.find((application) => sameAppId(application.absolutepath, id, platform));
  return ids.map(
    (id) =>
      find(metadata, id) ??
      find(catalog, id) ?? {
        absolutepath: id,
        name: id.split(/[\\/]/).pop() || id,
        deletable: false,
      },
  );
}
