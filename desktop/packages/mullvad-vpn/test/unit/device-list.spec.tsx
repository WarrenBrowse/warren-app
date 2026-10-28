import { renderToStaticMarkup } from 'react-dom/server';
import { ServerStyleSheet } from 'styled-components';
import { describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env.platform` at top level.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import { DeviceList } from '../../src/renderer/components/device-list';
import {
  deviceActivityLabel,
  devicePlatformLine,
  displayedDeviceName,
  IDevice,
  IDeviceList,
  isValidDeviceName,
  sortDevices,
} from '../../src/shared/devices';

const now = new Date('2026-10-28T15:00:00Z');
const RLO = String.fromCharCode(0x202e);
const PDI = String.fromCharCode(0x2069);

function device(id: string, overrides: Partial<IDevice> = {}): IDevice {
  return {
    id,
    name: `device ${id}`,
    platform: 'linux',
    appVersion: '1.1.56',
    registeredOn: '2026-10-01',
    lastSeenOn: '2026-10-20',
    thisDevice: false,
    ...overrides,
  };
}

function render(list: IDeviceList): string {
  const sheet = new ServerStyleSheet();
  try {
    return renderToStaticMarkup(
      sheet.collectStyles(
        <DeviceList
          list={list}
          now={now}
          locale="en-US"
          onDisconnect={() => {}}
          onLogOut={() => {}}
          onRename={() => {}}
        />,
      ),
    );
  } finally {
    sheet.seal();
  }
}

function count(haystack: string, needle: string): number {
  return haystack.split(needle).length - 1;
}

describe('the order of the device list', () => {
  it('puts this device first, then the others by last activity', () => {
    const sorted = sortDevices([
      device('old', { lastSeenOn: '2026-09-01' }),
      device('recent', { lastSeenOn: '2026-10-27' }),
      device('mine', { lastSeenOn: '2026-08-01', thisDevice: true }),
    ]);

    expect(sorted.map((d) => d.id)).to.deep.equal(['mine', 'recent', 'old']);
  });

  it('renders the rows in that order', () => {
    const html = render({
      devices: [
        device('old', { name: 'Old owl', lastSeenOn: '2026-09-01' }),
        device('mine', { name: 'Blue fox', thisDevice: true }),
        device('recent', { name: 'Red cat', lastSeenOn: '2026-10-27' }),
      ],
    });

    const positions = ['Blue fox', 'Red cat', 'Old owl'].map((name) => html.indexOf(name));
    expect(positions.every((position) => position >= 0)).toBe(true);
    expect([...positions].sort((a, b) => a - b)).to.deep.equal(positions);
  });
});

describe('a row of the device list', () => {
  it('marks this device alone, with Log out where the others have Disconnect', () => {
    const html = render({
      devices: [
        device('mine', { name: 'Blue fox', thisDevice: true }),
        device('b', { name: 'Red cat' }),
        device('c', { name: 'Old owl' }),
      ],
    });

    expect(count(html, 'This device')).toBe(1);
    expect(html.indexOf('This device')).toBeLessThan(html.indexOf('Red cat'));
    expect(count(html, '>Log out<')).toBe(1);
    expect(count(html, '>Disconnect<')).toBe(2);
  });

  it('shows no badge when the daemon marked no device as this one', () => {
    const html = render({ devices: [device('b'), device('c')] });

    expect(html).not.toContain('This device');
    expect(html).not.toContain('>Log out<');
  });

  it('displays a name without its bidirectional control characters', () => {
    const html = render({ devices: [device('b', { name: `Red${RLO} cat${PDI}` })] });

    expect(html).toContain('Red cat');
    expect(html).not.toContain(RLO);
    expect(html).not.toContain(PDI);
  });

  it('gives the platform and version, then the last activity', () => {
    const html = render({
      devices: [
        device('a', { platform: 'macos', appVersion: '1.1.56', lastSeenOn: '2026-10-28' }),
        device('b', { platform: 'android', appVersion: '1.1.40', lastSeenOn: '2026-10-27' }),
      ],
    });

    expect(html).toContain('macOS · 1.1.56');
    expect(html).toContain('Active today');
    expect(html).toContain('Android · 1.1.40');
    expect(html).toContain('Last active 10/27/26');
  });
});

describe('the older-client row', () => {
  it('appears, without an action, when the registry saw an older client', () => {
    const html = render({ devices: [device('b')], olderClientSeenOn: '2026-10-25' });

    expect(html).toContain('An older version of Warren');
    expect(html).toContain(
      'A device of this account runs a version of Warren older than 1.1.36. It no longer blocks your other devices. Update or uninstall it.',
    );
    expect(html).toContain('Last seen 10/25/26');
    expect(count(html, '>Disconnect<')).toBe(1);
  });

  it('is absent when the registry saw none', () => {
    expect(render({ devices: [device('b')] })).not.toContain('An older version of Warren');
  });
});

describe('the device helpers', () => {
  it('strips every bidirectional control character and keeps the rest', () => {
    const controls = [0x202a, 0x202b, 0x202c, 0x202d, 0x202e, 0x2066, 0x2067, 0x2068, 0x2069]
      .map((code) => String.fromCharCode(code))
      .join('');

    expect(displayedDeviceName(`a${controls}b مرحبا`)).to.equal('ab مرحبا');
  });

  it('reads the activity day in UTC', () => {
    expect(deviceActivityLabel('2026-10-28', new Date('2026-10-28T23:59:00Z'), 'en-US')).to.equal(
      'Active today',
    );
    expect(deviceActivityLabel('2026-10-28', new Date('2026-10-29T00:01:00Z'), 'en-US')).to.equal(
      'Last active 10/28/26',
    );
  });

  it('names an unknown platform without guessing', () => {
    expect(devicePlatformLine({ platform: 'other', appVersion: '2.0.0' })).to.equal(
      'Other platform · 2.0.0',
    );
  });

  it('accepts the names the registry keeps and refuses the others', () => {
    expect(isValidDeviceName('  Kitchen laptop ')).toBe(true);
    expect(isValidDeviceName('é'.repeat(40))).toBe(true);
    expect(isValidDeviceName('é'.repeat(41))).toBe(false);
    expect(isValidDeviceName('   ')).toBe(false);
    expect(isValidDeviceName(`tab${String.fromCharCode(9)}here`)).toBe(false);
  });
});
