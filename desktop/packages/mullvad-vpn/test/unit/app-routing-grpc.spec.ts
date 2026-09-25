import * as grpc from '@grpc/grpc-js';
import * as grpcTypes from 'management-interface/management-interface/grpc-types';
import { describe, expect, it } from 'vitest';

import { isAppExitLimitError } from '../../src/main/app-routing-errors';
import {
  convertFromAppRoutingSettings,
  convertFromDaemonEvent,
  convertToAppExit,
  convertToAppSplitMode,
} from '../../src/main/grpc-type-convertions';

const Mode = grpcTypes.AppSplitMode.Mode;
const State = grpcTypes.AppRouteStatus.State;
const Reason = grpcTypes.AppRouteStatus.UnavailableReason;

function exitChoice(country: string, city?: string) {
  const exit = new grpcTypes.ExitChoice().setCountry(country);
  if (city !== undefined) {
    exit.setCity(city);
  }
  return exit;
}

describe('convertFromAppRoutingSettings', () => {
  it('reads every field of the daemon settings', () => {
    const proto = new grpcTypes.AppRoutingSettings()
      .setSplitMode(Mode.INCLUDE_ONLY)
      .setExcludedAppsList(['/usr/bin/steam'])
      .setIncludedAppsList(['/usr/bin/firefox'])
      .setAppExitsEnabled(true)
      .setAppExitsList([
        new grpcTypes.AppExit().setApp('/usr/bin/slack').setExit(exitChoice('se', 'got')),
        new grpcTypes.AppExit().setApp('/usr/bin/chromium').setExit(exitChoice('de')),
      ]);

    expect(convertFromAppRoutingSettings(proto, undefined)).toEqual({
      splitMode: 'include-only',
      excludedApps: ['/usr/bin/steam'],
      includedApps: ['/usr/bin/firefox'],
      appExitsEnabled: true,
      appExits: [
        { app: '/usr/bin/slack', exit: { country: 'se', city: 'got' } },
        { app: '/usr/bin/chromium', exit: { country: 'de' } },
      ],
    });
  });

  it('maps the exclude mode', () => {
    const proto = new grpcTypes.AppRoutingSettings().setSplitMode(Mode.EXCLUDE);

    expect(convertFromAppRoutingSettings(proto, undefined).splitMode).toBe('exclude');
  });

  it('derives the settings of a daemon that predates app routing from split tunneling', () => {
    const splitTunnel = new grpcTypes.SplitTunnelSettings()
      .setEnableExclusions(true)
      .setAppsList(['C:\\Games\\game.exe']);

    expect(convertFromAppRoutingSettings(undefined, splitTunnel)).toEqual({
      splitMode: 'exclude',
      excludedApps: ['C:\\Games\\game.exe'],
      includedApps: [],
      appExitsEnabled: false,
      appExits: [],
    });
  });

  it('skips an app exit that arrives without its exit', () => {
    const proto = new grpcTypes.AppRoutingSettings().setAppExitsList([
      new grpcTypes.AppExit().setApp('/usr/bin/slack'),
    ]);

    expect(convertFromAppRoutingSettings(proto, undefined).appExits).toEqual([]);
  });
});

describe('convertFromDaemonEvent', () => {
  it('turns the app routes event into route statuses', () => {
    const list = new grpcTypes.AppRouteStatusList().setRoutesList([
      new grpcTypes.AppRouteStatus()
        .setExit(exitChoice('se'))
        .setState(State.CONNECTED)
        .setPublicIp('198.51.100.7')
        .setAppsList(['/usr/bin/slack']),
      new grpcTypes.AppRouteStatus()
        .setExit(exitChoice('de', 'ber'))
        .setState(State.UNAVAILABLE)
        .setReason(Reason.NO_TOKEN)
        .setAppsList(['/usr/bin/chromium']),
      new grpcTypes.AppRouteStatus()
        .setExit(exitChoice('fr'))
        .setState(State.CONNECTING)
        .setAppsList(['/usr/bin/steam']),
    ]);
    const event = new grpcTypes.DaemonEvent().setAppRoutes(list);

    expect(convertFromDaemonEvent(event)).toEqual({
      appRoutes: [
        {
          exit: { country: 'se' },
          state: 'connected',
          publicIp: '198.51.100.7',
          apps: ['/usr/bin/slack'],
        },
        {
          exit: { country: 'de', city: 'ber' },
          state: 'unavailable',
          reason: 'no-token',
          apps: ['/usr/bin/chromium'],
        },
        { exit: { country: 'fr' }, state: 'connecting', apps: ['/usr/bin/steam'] },
      ],
    });
  });

  it('names every unavailable reason', () => {
    const reasonOf = (reason: grpcTypes.AppRouteStatus.UnavailableReason) => {
      const list = new grpcTypes.AppRouteStatusList().setRoutesList([
        new grpcTypes.AppRouteStatus()
          .setExit(exitChoice('se'))
          .setState(State.UNAVAILABLE)
          .setReason(reason),
      ]);
      const event = convertFromDaemonEvent(new grpcTypes.DaemonEvent().setAppRoutes(list));
      return 'appRoutes' in event ? event.appRoutes[0].reason : undefined;
    };

    expect(reasonOf(Reason.TUNNEL_DOWN)).toBe('tunnel-down');
    expect(reasonOf(Reason.LIMIT_REACHED)).toBe('limit-reached');
    expect(reasonOf(Reason.NO_RELAY)).toBe('no-relay');
  });
});

describe('the requests sent to the daemon', () => {
  it('sends an exit without a city as a country alone', () => {
    const proto = convertToAppExit('/usr/bin/slack', { country: 'se' });

    expect(proto.getApp()).toBe('/usr/bin/slack');
    expect(proto.getExit()?.getCountry()).toBe('se');
    expect(proto.getExit()?.hasCity()).toBe(false);
  });

  it('sends the city of an exit', () => {
    expect(
      convertToAppExit('/usr/bin/slack', { country: 'se', city: 'got' }).getExit()?.getCity(),
    ).toBe('got');
  });

  it('maps each split mode', () => {
    expect(convertToAppSplitMode('off').getMode()).toBe(Mode.OFF);
    expect(convertToAppSplitMode('exclude').getMode()).toBe(Mode.EXCLUDE);
    expect(convertToAppSplitMode('include-only').getMode()).toBe(Mode.INCLUDE_ONLY);
  });
});

describe('isAppExitLimitError', () => {
  function serviceError(code: grpc.status, details?: string): grpc.ServiceError {
    const metadata = new grpc.Metadata();
    if (details !== undefined) {
      metadata.set('grpc-status-details-bin', Buffer.from(details));
    }
    return Object.assign(new Error('refused'), { code, details: 'refused', metadata });
  }

  it('recognizes the refusal of a third country', () => {
    expect(
      isAppExitLimitError(serviceError(grpc.status.FAILED_PRECONDITION, 'app_exit_limit')),
    ).toBe(true);
  });

  it('does not take another failed precondition for the limit', () => {
    expect(
      isAppExitLimitError(serviceError(grpc.status.FAILED_PRECONDITION, 'custom_list_exists')),
    ).toBe(false);
  });

  it('does not take the same details under another code for the limit', () => {
    expect(isAppExitLimitError(serviceError(grpc.status.INVALID_ARGUMENT, 'app_exit_limit'))).toBe(
      false,
    );
  });

  it('answers false for anything that is not a gRPC error', () => {
    expect(isAppExitLimitError(new Error('boom'))).toBe(false);
    expect(isAppExitLimitError(undefined)).toBe(false);
  });
});
