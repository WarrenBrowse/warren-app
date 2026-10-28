// The devices of the logged-in account (warren-core doc 115), as the daemon
// lists, renames and revokes them through the account's wallet. The list names
// the account's installs and the days they were used: it belongs on the
// account's own screens and never in a log.

import { messages } from './gettext';

export interface IDevice {
  id: string;
  name: string;
  // The registry's spelling: `android`, `ios`, `macos`, `windows`, `linux`,
  // `cli`, `browser`, or `other` for one the daemon does not know.
  platform: string;
  appVersion: string;
  // Days, `YYYY-MM-DD` in UTC: the registry keeps nothing finer.
  registeredOn: string;
  lastSeenOn: string;
  thisDevice: boolean;
}

export interface IDeviceList {
  devices: IDevice[];
  // The last day a device of this account took the place of a Warren client
  // too old to register; absent when that never happened.
  olderClientSeenOn?: string;
}

export type DeviceFailure =
  // No wallet is logged in.
  | 'no-wallet'
  // This device was revoked from another device of the account.
  | 'revoked'
  // A malformed id, or a name the registry refuses.
  | 'invalid'
  // The account has no such active device.
  | 'not-found'
  // Nothing happened, a retry can work.
  | 'unavailable';

export type DeviceListResult =
  | { type: 'listed'; list: IDeviceList }
  | { type: 'error'; reason: DeviceFailure };

export type DeviceActionResult = { type: 'done' } | { type: 'error'; reason: DeviceFailure };

export interface IDeviceRename {
  id: string;
  name: string;
}

// The longest name the registry keeps, in characters.
export const DEVICE_NAME_MAX_CHARS = 40;

// The detail the daemon puts on an UNAUTHENTICATED status for a revoked
// device (`DEVICE_REVOKED_DETAILS`), as opposed to no wallet at all.
export const DEVICE_REVOKED_DETAILS = 'device_revoked';

// Explicit embeddings, overrides and isolates (U+202A to U+202E, U+2066 to
// U+2069): a name another device chose could otherwise reorder the text
// around it.
const BIDI_CONTROLS = /[\u202A-\u202E\u2066-\u2069]/g;

/** The name as it is displayed, without bidirectional control characters. */
export function displayedDeviceName(name: string): string {
  return name.replace(BIDI_CONTROLS, '');
}

/** Whether the registry keeps this name: 1 to 40 characters once trimmed, no control character. */
export function isValidDeviceName(name: string): boolean {
  const characters = [...name.trim()];
  return (
    characters.length > 0 &&
    characters.length <= DEVICE_NAME_MAX_CHARS &&
    !characters.some(isControlCharacter)
  );
}

// Unicode's Cc category, which Rust's `char::is_control` tests in the daemon.
function isControlCharacter(character: string): boolean {
  const code = character.codePointAt(0)!;
  return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
}

/** This device first, then the others by last activity, the most recent first. */
export function sortDevices(devices: IDevice[]): IDevice[] {
  return [...devices].sort((a, b) => {
    if (a.thisDevice !== b.thisDevice) {
      return a.thisDevice ? -1 : 1;
    }
    if (a.lastSeenOn !== b.lastSeenOn) {
      // ISO days order as strings.
      return a.lastSeenOn < b.lastSeenOn ? 1 : -1;
    }
    return a.name.localeCompare(b.name);
  });
}

/** The UTC day of `now`, in the registry's spelling. */
export function utcDay(now: Date): string {
  return now.toISOString().slice(0, 10);
}

/** A registry day in the locale's short form. */
export function formatDeviceDay(day: string, locale: string): string {
  const date = new Date(`${day}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) {
    return day;
  }
  try {
    return new Intl.DateTimeFormat(locale, { dateStyle: 'short', timeZone: 'UTC' }).format(date);
  } catch {
    return new Intl.DateTimeFormat('en', { dateStyle: 'short', timeZone: 'UTC' }).format(date);
  }
}

/** "Active today", or the last day the device was seen. */
export function deviceActivityLabel(lastSeenOn: string, now: Date, locale: string): string {
  if (lastSeenOn === utcDay(now)) {
    // TRANSLATORS: Shown under a device of the account that was used today.
    return messages.pgettext('device-management', 'Active today');
  }
  return sprintfDate(
    // TRANSLATORS: Shown under a device of the account: the last day it was used.
    // TRANSLATORS: Available placeholder:
    // TRANSLATORS: %(date)s - a date in the short form of the language, e.g. 28/10/2026
    messages.pgettext('device-management', 'Last active %(date)s'),
    formatDeviceDay(lastSeenOn, locale),
  );
}

/** The day an older Warren client was last seen, for the older-client row. */
export function olderClientSeenLabel(day: string, locale: string): string {
  return sprintfDate(
    // TRANSLATORS: Shown on the row of an older Warren client of the account.
    // TRANSLATORS: Available placeholder:
    // TRANSLATORS: %(date)s - a date in the short form of the language, e.g. 28/10/2026
    messages.pgettext('device-management', 'Last seen %(date)s'),
    formatDeviceDay(day, locale),
  );
}

function sprintfDate(template: string, date: string): string {
  return template.replace('%(date)s', date);
}

/** The platform as the list names it. Product names stay as they are. */
export function devicePlatformLabel(platform: string): string {
  switch (platform) {
    case 'android':
      return 'Android';
    case 'ios':
      return 'iOS';
    case 'macos':
      return 'macOS';
    case 'windows':
      return 'Windows';
    case 'linux':
      return 'Linux';
    case 'cli':
      // TRANSLATORS: Platform of a device of the account: the Warren command-line client.
      return messages.pgettext('device-management', 'Command line');
    case 'browser':
      // TRANSLATORS: Platform of a device of the account: the Warren browser extension.
      return messages.pgettext('device-management', 'Browser extension');
    default:
      // TRANSLATORS: Platform of a device of the account that this version does not know.
      return messages.pgettext('device-management', 'Other platform');
  }
}

/** "<Platform> · <version>", the second line of a device row. */
export function devicePlatformLine(device: Pick<IDevice, 'platform' | 'appVersion'>): string {
  return `${devicePlatformLabel(device.platform)} · ${device.appVersion}`;
}
