import type {
  AppExit,
  AppRouteStatus,
  AppRouteUnavailableReason,
  AppRoutingSettings,
  AppSplitMode,
  ExitChoice,
} from './daemon-rpc-types';

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

export function appExitFor(
  routing: AppRoutingSettings,
  app: string,
  platform: Platform,
): ExitChoice | undefined {
  return routing.appExits.find((entry) => sameAppId(entry.app, app, platform))?.exit;
}

// Linux takes no list for either split mode: an app leaves or joins the VPN
// when Warren opens it (warren-exclude, warren-include), and is locked to it
// the same way.
function listsApply(platform: Platform): boolean {
  return platform !== 'linux';
}

// Locked to the VPN: never outside it, and blocked whenever it does not
// carry the app. The lock wins over an exclusion, as in the daemon.
export function isAppLocked(routing: AppRoutingSettings, app: string, platform: Platform): boolean {
  return listsApply(platform) && includesApp(routing.lockedApps, app, platform);
}

function isBypassing(routing: AppRoutingSettings, app: string, platform: Platform): boolean {
  return (
    routing.splitMode === 'exclude' &&
    includesApp(routing.excludedApps, app, platform) &&
    !isAppLocked(routing, app, platform)
  );
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

// The apps tunneled while include-only is on: the included apps, every app
// with a country in force, since choosing a country puts an app in the VPN,
// and every locked app.
export function effectiveIncludedApps(routing: AppRoutingSettings, platform: Platform): string[] {
  if (routing.splitMode !== 'include-only') {
    return [];
  }
  const apps = [...routing.includedApps];
  const candidates = [
    ...effectiveAppExits(routing, platform).map(({ app }) => app),
    ...(listsApply(platform) ? routing.lockedApps : []),
  ];
  for (const app of candidates) {
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
  includeOnly: boolean;
  // Set only while include-only is on, and never on Linux, where an app joins
  // the VPN when it is opened from Warren rather than from a list.
  vpnOnlyForCount?: number;
  appsWithOwnCountry: number;
  // The apps locked to the VPN, blocked whenever it does not carry them.
  lockedCount: number;
  // A route that cannot run for a reason of its own. A route waiting for the
  // main connection is not a fault; one waiting for a free route is, since
  // its apps do not leave from their country meanwhile.
  anyRouteUnavailable: boolean;
};

export function appRoutingSummary(
  routing: AppRoutingSettings,
  statuses: readonly AppRouteStatus[],
  platform: Platform,
): AppRoutingSummary {
  const includeOnly = routing.splitMode === 'include-only';
  return {
    includeOnly,
    vpnOnlyForCount:
      includeOnly && platform !== 'linux'
        ? effectiveIncludedApps(routing, platform).length
        : undefined,
    appsWithOwnCountry: effectiveAppExits(routing, platform).length,
    lockedCount: listsApply(platform) ? routing.lockedApps.length : 0,
    anyRouteUnavailable: statuses.some(
      (status) => status.state === 'unavailable' && status.reason !== 'tunnel-down',
    ),
  };
}

// The view shows one list: each app has a single route, and the apps without
// one follow the default. The daemon keeps its own model (a split mode, two
// lists and the countries, docs/app-routing.md section 1); these functions are
// the translation both ways, so the precedence stays the daemon's.
export type DefaultRoute = 'vpn' | 'direct';

export type AppRoute = { kind: 'vpn' } | { kind: 'direct' } | { kind: 'country'; exit: ExitChoice };

export type AppRule = { app: string; route: AppRoute; locked?: true };

export type RoutingOp =
  | { op: 'set-split-mode'; mode: AppSplitMode }
  | { op: 'add-excluded'; app: string }
  | { op: 'remove-excluded'; app: string }
  | { op: 'add-included'; app: string }
  | { op: 'remove-included'; app: string }
  | { op: 'set-exit'; app: string; exit: ExitChoice }
  | { op: 'clear-exit'; app: string }
  | { op: 'set-exits-enabled'; enabled: boolean }
  | { op: 'lock'; app: string }
  | { op: 'unlock'; app: string };

export function defaultRoute(routing: AppRoutingSettings): DefaultRoute {
  return routing.splitMode === 'include-only' ? 'direct' : 'vpn';
}

// The route the daemon gives an app, after its precedence rules.
export function appRouteFor(
  routing: AppRoutingSettings,
  app: string,
  platform: Platform,
): AppRoute {
  const lists = listsApply(platform);
  if (lists && isBypassing(routing, app, platform)) {
    return { kind: 'direct' };
  }
  const exit = routing.appExitsEnabled ? appExitFor(routing, app, platform) : undefined;
  if (exit !== undefined) {
    return { kind: 'country', exit };
  }
  if (routing.splitMode !== 'include-only') {
    return { kind: 'vpn' };
  }
  if (!lists) {
    return { kind: 'direct' };
  }
  return includesApp(routing.includedApps, app, platform) || isAppLocked(routing, app, platform)
    ? { kind: 'vpn' }
    : { kind: 'direct' };
}

function sameRoute(a: AppRoute, b: AppRoute): boolean {
  if (a.kind === 'country' && b.kind === 'country') {
    return sameExitChoice(a.exit, b.exit);
  }
  return a.kind === b.kind;
}

// The apps whose route differs from the default, and the locked apps, in the
// daemon's list order.
export function appRules(routing: AppRoutingSettings, platform: Platform): AppRule[] {
  const lists = listsApply(platform);
  const candidates: string[] = [];
  const consider = (apps: readonly string[]) => {
    for (const app of apps) {
      if (!includesApp(candidates, app, platform)) {
        candidates.push(app);
      }
    }
  };
  if (lists && routing.splitMode === 'exclude') consider(routing.excludedApps);
  if (lists && routing.splitMode === 'include-only') consider(routing.includedApps);
  if (routing.appExitsEnabled) consider(routing.appExits.map((entry) => entry.app));
  if (lists) consider(routing.lockedApps);

  const fallback = defaultRoute(routing);
  return candidates
    .map((app): AppRule => {
      const route = appRouteFor(routing, app, platform);
      return isAppLocked(routing, app, platform) ? { app, route, locked: true } : { app, route };
    })
    .filter((rule) => rule.locked || rule.route.kind !== fallback);
}

// The daemon calls that give one app `next`, in an order where every state in
// between routes the app the old way or the new way, never a third, and no
// other app moves: the calls are separate, so each state carries traffic.
export function planAppRoute(
  routing: AppRoutingSettings,
  app: string,
  next: AppRoute,
  platform: Platform,
): RoutingOp[] {
  const fallback = defaultRoute(routing);
  if (!listsApply(platform) && next.kind !== 'country' && next.kind !== fallback) {
    throw new Error('Linux opens an app outside or inside the VPN, it keeps no list');
  }
  if (sameRoute(appRouteFor(routing, app, platform), next)) {
    return [];
  }

  const ops: RoutingOp[] = [];
  // The daemon compares ids as stored, so an entry is removed under its own
  // spelling, which can differ in case from the id the view holds.
  const stored = (apps: readonly string[]) => apps.find((other) => sameAppId(other, app, platform));
  const excludedId = stored(routing.excludedApps);
  const includedId = stored(routing.includedApps);
  const exitId = stored(routing.appExits.map((entry) => entry.app));
  const excluded = excludedId !== undefined;
  const included = includedId !== undefined;
  const excludedLeft = routing.excludedApps.filter((other) => !sameAppId(other, app, platform));
  const lockedId = stored(routing.lockedApps);

  // Bypass or the countries switched on again must not bring back entries
  // saved while they were off: nobody sees them in the list.
  const turnCountriesOn = () => {
    if (routing.appExitsEnabled) return;
    for (const entry of routing.appExits) {
      if (!sameAppId(entry.app, app, platform)) ops.push({ op: 'clear-exit', app: entry.app });
    }
  };

  switch (next.kind) {
    case 'direct':
      // The lock goes first: it holds the app on its old route whatever else
      // changes, which would pass it through a third one on the way.
      if (lockedId !== undefined) ops.push({ op: 'unlock', app: lockedId });
      // Only with the VPN as the default: direct is otherwise "no rule".
      if (fallback === 'direct') {
        if (includedId !== undefined) ops.push({ op: 'remove-included', app: includedId });
        if (exitId !== undefined) ops.push({ op: 'clear-exit', app: exitId });
        break;
      }
      if (routing.splitMode !== 'exclude') {
        for (const other of excludedLeft) ops.push({ op: 'remove-excluded', app: other });
      }
      if (!excluded) ops.push({ op: 'add-excluded', app });
      if (routing.splitMode !== 'exclude') ops.push({ op: 'set-split-mode', mode: 'exclude' });
      // Bypass wins over a country, so the country goes once the app bypasses.
      if (exitId !== undefined) ops.push({ op: 'clear-exit', app: exitId });
      break;

    case 'country':
      turnCountriesOn();
      ops.push({ op: 'set-exit', app, exit: next.exit });
      if (!routing.appExitsEnabled) ops.push({ op: 'set-exits-enabled', enabled: true });
      // The country is in force from here, so the app leaves its list.
      if (includedId !== undefined) ops.push({ op: 'remove-included', app: includedId });
      if (excludedId !== undefined) {
        ops.push({ op: 'remove-excluded', app: excludedId });
        if (routing.splitMode === 'exclude' && excludedLeft.length === 0) {
          ops.push({ op: 'set-split-mode', mode: 'off' });
        }
      }
      break;

    case 'vpn':
      if (fallback === 'direct') {
        // Included first: an app with a country stays in the VPN throughout.
        if (!included) ops.push({ op: 'add-included', app });
        if (exitId !== undefined) ops.push({ op: 'clear-exit', app: exitId });
        break;
      }
      // Bypass wins over a country, so the country goes while it still bypasses.
      if (exitId !== undefined) ops.push({ op: 'clear-exit', app: exitId });
      if (excludedId !== undefined) {
        ops.push({ op: 'remove-excluded', app: excludedId });
        if (routing.splitMode === 'exclude' && excludedLeft.length === 0) {
          ops.push({ op: 'set-split-mode', mode: 'off' });
        }
      }
      break;
  }
  return ops;
}

// The daemon calls that lock `app` to the VPN or lift its lock, without
// moving it: lifting a lock keeps the route the lock was giving the app.
export function planAppLock(
  routing: AppRoutingSettings,
  app: string,
  locked: boolean,
  platform: Platform,
): RoutingOp[] {
  if (!listsApply(platform)) {
    throw new Error('Linux locks an app to the VPN when it opens it, it keeps no list');
  }
  if (isAppLocked(routing, app, platform) === locked) {
    return [];
  }
  const route = appRouteFor(routing, app, platform);
  if (locked) {
    if (route.kind === 'direct') {
      throw new Error('An app outside the VPN cannot be locked to it');
    }
    return [{ op: 'lock', app }];
  }

  const stored = (apps: readonly string[]) => apps.find((other) => sameAppId(other, app, platform));
  const ops: RoutingOp[] = [];
  if (
    route.kind === 'vpn' &&
    routing.splitMode === 'include-only' &&
    stored(routing.includedApps) === undefined
  ) {
    ops.push({ op: 'add-included', app });
  }
  const excludedId = stored(routing.excludedApps);
  if (routing.splitMode === 'exclude' && excludedId !== undefined) {
    ops.push({ op: 'remove-excluded', app: excludedId });
    if (routing.excludedApps.every((other) => sameAppId(other, app, platform))) {
      ops.push({ op: 'set-split-mode', mode: 'off' });
    }
  }
  ops.push({ op: 'unlock', app: stored(routing.lockedApps) ?? app });
  return ops;
}

// The daemon calls that switch what apps without a rule do. The countries
// and the locks stay; the two lists are emptied, since a list kept from an earlier choice
// would come back as rules nobody just made.
export function planDefaultRoute(
  routing: AppRoutingSettings,
  next: DefaultRoute,
  platform: Platform,
): RoutingOp[] {
  if (defaultRoute(routing) === next) {
    return [];
  }
  const clearLists: RoutingOp[] = [
    ...routing.excludedApps.map((app): RoutingOp => ({ op: 'remove-excluded', app })),
    ...(listsApply(platform) ? routing.includedApps : []).map(
      (app): RoutingOp => ({ op: 'remove-included', app }),
    ),
  ];
  // Toward the VPN the mode goes first and toward direct it goes last, so
  // every app is in the VPN while the lists are emptied.
  return next === 'vpn'
    ? [{ op: 'set-split-mode', mode: 'off' }, ...clearLists]
    : [...clearLists, { op: 'set-split-mode', mode: 'include-only' }];
}

// Whether `actual` routes every app the way `expected` does, whatever the
// order of its lists: what tells the view that the daemon has applied a plan.
export function routingReflects(
  actual: AppRoutingSettings,
  expected: AppRoutingSettings,
  platform: Platform,
): boolean {
  if (defaultRoute(actual) !== defaultRoute(expected)) {
    return false;
  }
  const apps = [...appRules(actual, platform), ...appRules(expected, platform)].map(
    (rule) => rule.app,
  );
  return apps.every(
    (app) =>
      sameRoute(appRouteFor(actual, app, platform), appRouteFor(expected, app, platform)) &&
      isAppLocked(actual, app, platform) === isAppLocked(expected, app, platform),
  );
}

// What the daemon holds once it has applied `ops`, for the view to reason
// about a change before it is made.
export function applyRoutingOps(
  routing: AppRoutingSettings,
  ops: readonly RoutingOp[],
  platform: Platform,
): AppRoutingSettings {
  const without = (apps: readonly string[], app: string) =>
    apps.filter((other) => !sameAppId(other, app, platform));
  const withApp = (apps: readonly string[], app: string) =>
    includesApp(apps, app, platform) ? [...apps] : [...apps, app];

  let next: AppRoutingSettings = { ...routing };
  for (const op of ops) {
    switch (op.op) {
      case 'set-split-mode':
        next = { ...next, splitMode: op.mode };
        break;
      case 'add-excluded':
        next = { ...next, excludedApps: withApp(next.excludedApps, op.app) };
        break;
      case 'remove-excluded':
        next = { ...next, excludedApps: without(next.excludedApps, op.app) };
        break;
      case 'add-included':
        next = { ...next, includedApps: withApp(next.includedApps, op.app) };
        break;
      case 'remove-included':
        next = { ...next, includedApps: without(next.includedApps, op.app) };
        break;
      case 'set-exit':
        next = {
          ...next,
          appExits: [
            ...next.appExits.filter((entry) => !sameAppId(entry.app, op.app, platform)),
            { app: op.app, exit: op.exit },
          ],
        };
        break;
      case 'clear-exit':
        next = {
          ...next,
          appExits: next.appExits.filter((entry) => !sameAppId(entry.app, op.app, platform)),
        };
        break;
      case 'set-exits-enabled':
        next = { ...next, appExitsEnabled: op.enabled };
        break;
      case 'lock':
        next = {
          ...next,
          excludedApps: without(next.excludedApps, op.app),
          lockedApps: withApp(next.lockedApps, op.app),
        };
        break;
      case 'unlock':
        next = { ...next, lockedApps: without(next.lockedApps, op.app) };
        break;
    }
  }
  return next;
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

// Whether this platform can hold a lock, which the view does not offer
// otherwise. Windows holds it in its filtering engine; macOS needs a content
// filter Warren does not ship yet, and Linux locks an app when Warren opens it.
export function appLockSupported(platform: Platform): boolean {
  return platform !== 'darwin' && platform !== 'linux';
}

export type AppRouteLine =
  | { kind: 'blocked' }
  | { kind: 'paused' }
  | { kind: 'bypassed' }
  | { kind: 'waiting' }
  | { kind: 'connecting' }
  | { kind: 'connected'; publicIp?: string }
  | { kind: 'unavailable'; reason?: AppRouteUnavailableReason };

// The status line under a rule: a locked app that the VPN does not carry is
// blocked, whatever its route; an app with a country says where its route
// stands; any other rule has none.
export function ruleLine(
  routing: AppRoutingSettings,
  statuses: readonly AppRouteStatus[],
  app: string,
  platform: Platform,
  tunnelConnected: boolean,
): AppRouteLine | undefined {
  if (!tunnelConnected && isAppLocked(routing, app, platform)) {
    return { kind: 'blocked' };
  }
  if (appRouteFor(routing, app, platform).kind !== 'country') {
    return undefined;
  }
  return appRouteLine(routing, statuses, app, platform);
}

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
