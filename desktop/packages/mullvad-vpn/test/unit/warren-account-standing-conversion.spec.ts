import * as grpcTypes from 'management-interface/management-interface/grpc-types';
import { describe, expect, it } from 'vitest';

import {
  convertFromDaemonEvent,
  convertFromNatPmpStatus,
  convertFromWarrenAccountStanding,
} from '../../src/main/grpc-type-convertions';

function strike(): grpcTypes.WarrenAccountStrike {
  const strike = new grpcTypes.WarrenAccountStrike();
  strike.setDayUnixSecs(1_790_208_000);
  strike.setCategory(grpcTypes.WarrenAbuseCategory.WARREN_ABUSE_COPYRIGHT);
  strike.setExitCountry('FI');
  strike.setPort(51413);
  strike.setCaseReference('PF-2026-0042');
  return strike;
}

describe('the account standing from the daemon', () => {
  it('keeps every strike field, the threshold, the window and the ban', () => {
    const ban = new grpcTypes.WarrenAccountBan();
    ban.setReason(grpcTypes.WarrenBanReason.WARREN_BAN_PORT_FORWARDING_ABUSE);
    ban.setLapsesAtUnixSecs(1_821_744_000);
    const standing = new grpcTypes.WarrenAccountStanding();
    standing.setStrikesList([strike()]);
    standing.setThreshold(3);
    standing.setWindowDays(90);
    standing.setBan(ban);

    expect(convertFromWarrenAccountStanding(standing)).to.deep.equal({
      strikes: [
        {
          dayUnixSecs: 1_790_208_000,
          category: 'copyright',
          exitCountry: 'FI',
          port: 51413,
          caseReference: 'PF-2026-0042',
        },
      ],
      threshold: 3,
      windowDays: 90,
      ban: {
        reason: 'port-forwarding-abuse',
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: 1_821_744_000,
      },
    });
  });

  it('reads an absent standing as unknown', () => {
    expect(convertFromWarrenAccountStanding(undefined)).to.equal(null);
  });

  it('reads a category this build does not know as other', () => {
    const unknown = strike();
    unknown.setCategory(99 as grpcTypes.WarrenAbuseCategory);
    const standing = new grpcTypes.WarrenAccountStanding();
    standing.setStrikesList([unknown]);

    expect(convertFromWarrenAccountStanding(standing)?.strikes[0].category).to.equal('other');
  });

  it('turns a strike notice into its own event', () => {
    const notice = new grpcTypes.WarrenAccountStrikeNotice();
    notice.setStrike(strike());
    notice.setOrdinal(2);
    notice.setThreshold(3);
    const event = new grpcTypes.DaemonEvent();
    event.setNewAccountStrike(notice);

    const converted = convertFromDaemonEvent(event);

    expect('newAccountStrike' in converted && converted.newAccountStrike.ordinal).to.equal(2);
    expect('newAccountStrike' in converted && converted.newAccountStrike.strike.port).to.equal(
      51413,
    );
  });
});

describe('a refused port-forwarding rule', () => {
  it('says it had no entitlement, and when the daemon asks again', () => {
    const mapping = new grpcTypes.NatPmpStatus.Mapping();
    mapping.setState(grpcTypes.NatPmpStatus.State.FAILED);
    mapping.setErrorReason(grpcTypes.NatPmpStatus.ErrorReason.NO_ENTITLEMENT);
    mapping.setRetryAfterSecs(30);
    const status = new grpcTypes.NatPmpStatus();
    status.setMappingsList([mapping]);

    expect(convertFromNatPmpStatus(status).mappings[0].status).to.deep.include({
      state: 'failed',
      errorReason: 'no-entitlement',
      retryAfterSecs: 30,
    });
  });
});
