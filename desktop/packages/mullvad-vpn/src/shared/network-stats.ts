// The public network transparency snapshot (`GET /v1/network/stats`,
// warren-core doc 106), as the renderer reads it.
//
// The daemon passes the body through untouched after checking its version, so
// this parser is the only place that knows the wire shape. It follows the
// contract's evolution rules: unknown fields are ignored, an unknown load band
// or driver reads as `unknown`, and an optional figure the server left out stays
// absent instead of turning into a zero that the UI would then display.

export type LoadLevel = 'low' | 'moderate' | 'high' | 'saturated' | 'unknown';
export type LoadDriver = 'bandwidth' | 'cpu' | 'unknown';

export interface ExitHistoryPoint {
  t: number;
  connected: number;
  throughputBps: number;
  loadPercent?: number;
}

export interface FleetHistoryPoint {
  t: number;
  connected: number;
  throughputBps: number;
}

export interface ExitStats {
  exitId: string;
  name?: string;
  country: string;
  city: string;
  online: boolean;
  // Whether the exit carried enough people for its live figures to be
  // published. When false only `loadLevel` means anything.
  live: boolean;
  connected: number;
  downloadBps: number;
  uploadBps: number;
  capacityBps?: number;
  loadPercent?: number;
  loadLevel: LoadLevel;
  loadDriver?: LoadDriver;
  cpuPercent?: number;
  uptimeSecs?: number;
  history: ExitHistoryPoint[];
}

export interface NetworkStats {
  environment: string;
  generatedAt: number;
  windowSecs: number;
  exitUsersRounding: number;
  exitLiveThreshold: number;
  users: {
    accountsTotal: number;
    subscribersActive: number;
    connected: number;
  };
  fleet: {
    exitsOnline: number;
    exitsTotal: number;
    downloadBps: number;
    uploadBps: number;
    capacityBps: number;
    loadPercent: number;
    transferred24hBytes: number;
    peakConnected24h: number;
    peakThroughput24hBps: number;
  };
  exits: ExitStats[];
  history: FleetHistoryPoint[];
}

export interface ExitHostname {
  exitId: string;
  hostname: string;
}

// What the main process hands the renderer for one request.
export type WarrenNetworkStatsResult =
  | { result: 'ok'; snapshotJson: string; exitHostnames: ExitHostname[] }
  // The API predates the endpoint: nothing to show, and nothing to retry soon.
  | { result: 'unsupported' }
  | { result: 'error' };

const SUPPORTED_VERSION = 1;
const EXIT_ID = /^[0-9a-f]{32}$/i;
// The window range the server accepts (doc 106 knobs). A snapshot claiming
// another window is polled as if it said the nearest bound.
const MIN_WINDOW_SECS = 30;
const MAX_WINDOW_SECS = 3600;
// A snapshot older than this many windows is kept on screen, greyed.
const STALE_AFTER_WINDOWS = 3;

type Json = Record<string, unknown>;

class Malformed extends Error {}

function isObject(value: unknown): value is Json {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function optionalNumber(object: Json, key: string): number | undefined {
  const value = object[key];
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : undefined;
}

function requiredNumber(object: Json, key: string): number {
  const value = optionalNumber(object, key);
  if (value === undefined) {
    throw new Malformed(key);
  }
  return value;
}

function optionalPercent(object: Json, key: string): number | undefined {
  const value = optionalNumber(object, key);
  return value === undefined ? undefined : Math.min(100, value);
}

function requiredString(object: Json, key: string): string {
  const value = object[key];
  if (typeof value !== 'string') {
    throw new Malformed(key);
  }
  return value;
}

function requiredObject(object: Json, key: string): Json {
  const value = object[key];
  if (!isObject(value)) {
    throw new Malformed(key);
  }
  return value;
}

function arrayOf<T>(object: Json, key: string, parse: (entry: Json) => T): T[] {
  const value = object[key];
  if (!Array.isArray(value)) {
    throw new Malformed(key);
  }
  // One bad entry costs that entry, never the whole snapshot.
  return value.flatMap((entry) => {
    if (!isObject(entry)) {
      return [];
    }
    try {
      return [parse(entry)];
    } catch (error) {
      if (error instanceof Malformed) {
        return [];
      }
      throw error;
    }
  });
}

function loadLevel(value: unknown): LoadLevel {
  switch (value) {
    case 'low':
    case 'moderate':
    case 'high':
    case 'saturated':
      return value;
    default:
      return 'unknown';
  }
}

function loadDriver(value: unknown): LoadDriver | undefined {
  switch (value) {
    case undefined:
    case null:
      return undefined;
    case 'bandwidth':
    case 'cpu':
      return value;
    default:
      return 'unknown';
  }
}

function parseExit(exit: Json): ExitStats {
  const exitId = requiredString(exit, 'exit_id');
  if (!EXIT_ID.test(exitId)) {
    throw new Malformed('exit_id');
  }
  const name = exit.name;
  return {
    exitId: exitId.toLowerCase(),
    name: typeof name === 'string' && name.length > 0 ? name : undefined,
    country: requiredString(exit, 'country'),
    city: requiredString(exit, 'city'),
    online: exit.online === true,
    live: exit.live === true,
    connected: optionalNumber(exit, 'connected') ?? 0,
    downloadBps: optionalNumber(exit, 'download_bps') ?? 0,
    uploadBps: optionalNumber(exit, 'upload_bps') ?? 0,
    capacityBps: optionalNumber(exit, 'capacity_bps'),
    loadPercent: optionalPercent(exit, 'load_percent'),
    loadLevel: loadLevel(exit.load_level),
    loadDriver: loadDriver(exit.load_driver),
    cpuPercent: optionalPercent(exit, 'cpu_percent'),
    uptimeSecs: optionalNumber(exit, 'uptime_secs'),
    history: Array.isArray(exit.history)
      ? arrayOf(exit, 'history', (point) => ({
          t: requiredNumber(point, 't'),
          connected: requiredNumber(point, 'connected'),
          throughputBps: requiredNumber(point, 'throughput_bps'),
          loadPercent: optionalPercent(point, 'load_percent'),
        }))
      : [],
  };
}

function parse(json: Json): NetworkStats {
  if (json.version !== SUPPORTED_VERSION) {
    throw new Malformed('version');
  }
  const users = requiredObject(json, 'users');
  const fleet = requiredObject(json, 'fleet');
  const windowSecs = requiredNumber(json, 'window_secs');
  if (windowSecs === 0) {
    throw new Malformed('window_secs');
  }
  return {
    environment: typeof json.environment === 'string' ? json.environment : '',
    generatedAt: requiredNumber(json, 'generated_at'),
    windowSecs,
    exitUsersRounding: Math.max(1, requiredNumber(json, 'exit_users_rounding')),
    exitLiveThreshold: requiredNumber(json, 'exit_live_threshold'),
    users: {
      accountsTotal: requiredNumber(users, 'accounts_total'),
      subscribersActive: requiredNumber(users, 'subscribers_active'),
      connected: requiredNumber(users, 'connected'),
    },
    fleet: {
      exitsOnline: requiredNumber(fleet, 'exits_online'),
      exitsTotal: requiredNumber(fleet, 'exits_total'),
      downloadBps: requiredNumber(fleet, 'download_bps'),
      uploadBps: requiredNumber(fleet, 'upload_bps'),
      capacityBps: requiredNumber(fleet, 'capacity_bps'),
      loadPercent: Math.min(100, requiredNumber(fleet, 'load_percent')),
      transferred24hBytes: requiredNumber(fleet, 'transferred_24h_bytes'),
      peakConnected24h: requiredNumber(fleet, 'peak_connected_24h'),
      peakThroughput24hBps: requiredNumber(fleet, 'peak_throughput_24h_bps'),
    },
    exits: arrayOf(json, 'exits', parseExit),
    history: arrayOf(json, 'history', (point) => ({
      t: requiredNumber(point, 't'),
      connected: requiredNumber(point, 'connected'),
      throughputBps: requiredNumber(point, 'throughput_bps'),
    })),
  };
}

/** The snapshot in `json`, or `undefined` when it is not one this app can read. */
export function parseNetworkStats(json: string): NetworkStats | undefined {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    return undefined;
  }
  if (!isObject(value)) {
    return undefined;
  }
  try {
    return parse(value);
  } catch (error) {
    if (error instanceof Malformed) {
      return undefined;
    }
    throw error;
  }
}

export type ExitDisplayMode = 'offline' | 'band' | 'live';

/**
 * What an exit may show. `band` is the common case on a young network: under
 * the live threshold the snapshot carries the exit's load band and nothing else.
 */
export function exitDisplayMode(exit: ExitStats): ExitDisplayMode {
  if (!exit.online) {
    return 'offline';
  }
  return exit.live ? 'live' : 'band';
}

/**
 * The people count of one exit, which is never exact: a floor to the rounding
 * step when live, and "fewer than the threshold" when it is not.
 */
export function exitUsersLabel(
  exit: ExitStats,
  stats: Pick<NetworkStats, 'exitUsersRounding' | 'exitLiveThreshold'>,
): string {
  if (!exit.live) {
    return `< ${stats.exitLiveThreshold}`;
  }
  if (exit.connected < stats.exitUsersRounding) {
    return `< ${stats.exitUsersRounding}`;
  }
  return `${exit.connected}+`;
}

/** Every exit of the snapshot the relay list knows, keyed by relay hostname. */
export function joinExitsByHostname(
  stats: NetworkStats,
  exitHostnames: ExitHostname[],
): Map<string, ExitStats> {
  const byId = new Map(stats.exits.map((exit) => [exit.exitId, exit]));
  const joined = new Map<string, ExitStats>();
  for (const { exitId, hostname } of exitHostnames) {
    const exit = byId.get(exitId.toLowerCase());
    if (exit) {
      joined.set(hostname, exit);
    }
  }
  return joined;
}

// One decimal under ten, none from ten up. A value that rounds up to ten
// loses its decimal too.
function displayedDecimals(scaled: number): number {
  return Math.round(scaled * 10) / 10 < 10 ? 1 : 0;
}

function rounded(scaled: number): number {
  const factor = 10 ** displayedDecimals(scaled);
  return Math.round(scaled * factor) / factor;
}

function formatSi(value: number, units: readonly string[], locale: string): string {
  let scaled = Math.max(0, value);
  let unit = 0;
  while (unit < units.length - 1 && rounded(scaled) >= 1000) {
    scaled /= 1000;
    unit += 1;
  }
  // The base unit is never fractional.
  const decimals = unit > 0 ? displayedDecimals(scaled) : 0;
  const number = new Intl.NumberFormat(locale, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(scaled);
  return `${number} ${units[unit]}`;
}

const BIT_RATE_UNITS = ['bit/s', 'kbit/s', 'Mbit/s', 'Gbit/s', 'Tbit/s'] as const;
const BYTE_UNITS = ['B', 'kB', 'MB', 'GB', 'TB', 'PB'] as const;

export function formatBitsPerSecond(bps: number, locale: string): string {
  return formatSi(bps, BIT_RATE_UNITS, locale);
}

export function formatBytes(bytes: number, locale: string): string {
  return formatSi(bytes, BYTE_UNITS, locale);
}

export function formatPercent(percent: number, locale: string): string {
  return new Intl.NumberFormat(locale, { style: 'percent', maximumFractionDigits: 0 }).format(
    percent / 100,
  );
}

/** Uptime is published floored to whole days; so is what the UI says. */
export function uptimeDays(uptimeSecs: number): number {
  return Math.floor(uptimeSecs / 86_400);
}

type Window = Pick<NetworkStats, 'generatedAt' | 'windowSecs'>;

/** Seconds since the window the snapshot describes closed. */
export function snapshotAgeSecs(stats: Window, nowMs: number): number {
  return Math.max(0, Math.floor(nowMs / 1000 - stats.generatedAt));
}

export function snapshotIsStale(stats: Window, nowMs: number): boolean {
  return snapshotAgeSecs(stats, nowMs) > STALE_AFTER_WINDOWS * stats.windowSecs;
}

/** One request per window: polling faster returns the same snapshot. */
export function pollIntervalMs(windowSecs: number): number {
  return Math.min(MAX_WINDOW_SECS, Math.max(MIN_WINDOW_SECS, windowSecs)) * 1000;
}
