import { describe, expect, it } from 'vitest';

import {
  appliedPort,
  blockedChipReason,
  clipboardTextForMapping,
  clockSkewOf,
  mappingForRule,
  protocolLabel,
  protocolsOverlap,
  rulePort,
} from '../../src/renderer/features/port-forwarding/mapping';
import { NatPmpMapping, NatPmpProto, NatPmpRule } from '../../src/shared/daemon-rpc-types';

function mappedMapping(
  internalPort: number,
  protocol: NatPmpProto,
  externalPort: number,
): NatPmpMapping {
  return {
    internalPort,
    protocol,
    status: { state: 'mapped', externalPort, lifetimeGrantedSecs: 3599, windowResetSecs: 60 },
  };
}

describe('mappingForRule', () => {
  // The regression that stranded the UI on "pending…": an auto rule carries
  // internalPort 0 and a non-zero suggestedExternalPort, and the daemon reports
  // the granted mapping with internalPort 0. Matching on rulePort() (which falls
  // back to the suggested port) would look for internalPort === 55484 and miss
  // the real mapping (internalPort 0).
  it('matches an auto rule (internalPort 0) to the daemon mapping keyed by internalPort 0', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 55484,
      internalPort: 0,
    };
    const mappings = [mappedMapping(0, NatPmpProto.udp, 55484)];

    const found = mappingForRule(mappings, rule);

    expect(found).toBeDefined();
    expect(found?.status.state).toBe('mapped');
  });

  it('matches an explicit-port rule by its internalPort', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.tcp,
      suggestedExternalPort: 50000,
      internalPort: 50000,
    };
    const mappings = [mappedMapping(50000, NatPmpProto.tcp, 50000)];

    expect(mappingForRule(mappings, rule)).toBeDefined();
  });

  it('does not match a mapping of a different protocol with the same internalPort', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 0,
      internalPort: 0,
    };
    const mappings = [mappedMapping(0, NatPmpProto.tcp, 55484)];

    expect(mappingForRule(mappings, rule)).toBeUndefined();
  });

  it('returns undefined when no mapping exists yet', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 49999,
      internalPort: 0,
    };

    expect(mappingForRule([], rule)).toBeUndefined();
  });
});

describe('appliedPort', () => {
  // After "assign a free port" a rule becomes { internalPort: P, suggested: 0 }
  // and the exit grants a NEW public port Q. The input must show Q (the applied
  // port), not P, so it can never diverge from the status on its right.
  it('shows the granted external port for an auto rule that is mapped', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 0,
      internalPort: 50000,
    };
    const mappings = [mappedMapping(50000, NatPmpProto.udp, 51234)];

    expect(appliedPort(rule, mappingForRule(mappings, rule))).toBe(51234);
  });

  it('falls back to the rule port for an auto rule not yet mapped', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 0,
      internalPort: 50000,
    };

    expect(appliedPort(rule, undefined)).toBe(50000);
  });

  it('shows the manual rule own port even when mapped (never the granted port)', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 52000,
      internalPort: 52000,
    };
    const mappings = [mappedMapping(52000, NatPmpProto.udp, 52000)];

    expect(appliedPort(rule, mappingForRule(mappings, rule))).toBe(52000);
  });

  it('falls back to the rule port for an auto rule whose mapping failed', () => {
    const rule: NatPmpRule = {
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 0,
      internalPort: 50000,
    };
    const failed: NatPmpMapping = {
      internalPort: 50000,
      protocol: NatPmpProto.udp,
      status: { state: 'failed', errorMessage: 'taken', errorReason: 'suggested-port-in-use' },
    };

    expect(appliedPort(rule, failed)).toBe(50000);
  });
});

describe('rulePort', () => {
  it('uses the internal port when set', () => {
    expect(
      rulePort({ protocol: NatPmpProto.udp, suggestedExternalPort: 1, internalPort: 50000 }),
    ).toBe(50000);
  });

  it('falls back to the suggested external port for an auto rule', () => {
    expect(
      rulePort({ protocol: NatPmpProto.udp, suggestedExternalPort: 55484, internalPort: 0 }),
    ).toBe(55484);
  });
});

describe('protocolsOverlap', () => {
  it('matches identical protocols', () => {
    expect(protocolsOverlap(NatPmpProto.udp, NatPmpProto.udp)).toBe(true);
    expect(protocolsOverlap(NatPmpProto.tcp, NatPmpProto.tcp)).toBe(true);
  });

  it('does not match disjoint single protocols', () => {
    expect(protocolsOverlap(NatPmpProto.udp, NatPmpProto.tcp)).toBe(false);
  });

  it('both overlaps every protocol choice on the same port', () => {
    expect(protocolsOverlap(NatPmpProto.both, NatPmpProto.udp)).toBe(true);
    expect(protocolsOverlap(NatPmpProto.tcp, NatPmpProto.both)).toBe(true);
    expect(protocolsOverlap(NatPmpProto.both, NatPmpProto.both)).toBe(true);
  });
});

describe('protocolLabel', () => {
  it('labels the dual-proto pair as TCP+UDP', () => {
    expect(protocolLabel(NatPmpProto.both)).toBe('TCP+UDP');
    expect(protocolLabel(NatPmpProto.udp)).toBe('UDP');
    expect(protocolLabel(NatPmpProto.tcp)).toBe('TCP');
  });
});

describe('clipboardTextForMapping', () => {
  // What lands in the clipboard is pasted straight into a torrent client's
  // "incoming port" field, so it carries the decimal port and nothing else:
  // no protocol, no label, no whitespace.
  it('copies the granted public port on its own', () => {
    expect(clipboardTextForMapping(mappedMapping(58291, NatPmpProto.both, 58291))).toBe('58291');
  });

  it('copies the granted public port, not the internal one', () => {
    expect(clipboardTextForMapping(mappedMapping(50000, NatPmpProto.udp, 51234))).toBe('51234');
  });

  it('has nothing to copy while the rule holds no grant', () => {
    const requesting: NatPmpMapping = {
      internalPort: 50000,
      protocol: NatPmpProto.udp,
      status: { state: 'requesting' },
    };
    const failed: NatPmpMapping = {
      internalPort: 50000,
      protocol: NatPmpProto.udp,
      status: { state: 'failed', errorMessage: 'taken', errorReason: 'suggested-port-in-use' },
    };

    expect(clipboardTextForMapping(requesting)).toBeUndefined();
    expect(clipboardTextForMapping(failed)).toBeUndefined();
    expect(clipboardTextForMapping(undefined)).toBeUndefined();
  });
});

function failedMapping(
  errorReason: 'clock-skew' | 'no-entitlement' | 'suggested-port-in-use',
  clockOffsetSecs?: number,
): NatPmpMapping {
  return {
    internalPort: 6881,
    protocol: NatPmpProto.both,
    status: { state: 'failed', errorMessage: '', errorReason, retryAfterSecs: 30, clockOffsetSecs },
  };
}

describe('clockSkewOf', () => {
  // Forum topic 219: a Windows clock 91 s fast, every mint refused, and the
  // rule said the entitlements were used up.
  it('says a clock 91 s fast is ahead by 91 s', () => {
    expect(clockSkewOf(failedMapping('clock-skew', -91))).to.deep.equal({
      seconds: 91,
      direction: 'ahead',
    });
  });

  it('says a clock the servers are ahead of is behind', () => {
    expect(clockSkewOf(failedMapping('clock-skew', 1200))).to.deep.equal({
      seconds: 1200,
      direction: 'behind',
    });
  });

  it('still names the clock when the refusal did not say by how much', () => {
    expect(clockSkewOf(failedMapping('clock-skew'))).to.deep.equal({ seconds: undefined });
  });

  it('is nothing for any other refusal', () => {
    expect(clockSkewOf(failedMapping('no-entitlement'))).to.equal(undefined);
    expect(clockSkewOf(undefined)).to.equal(undefined);
  });
});

describe('blockedChipReason', () => {
  it('names the clock over a generic block, since only the clock has a remedy here', () => {
    expect(
      blockedChipReason([failedMapping('no-entitlement'), failedMapping('clock-skew', -91)]),
    ).to.equal('clock-skew');
  });

  it('puts a port conflict first, the one the user can fix in this screen', () => {
    expect(
      blockedChipReason([failedMapping('clock-skew', -91), failedMapping('suggested-port-in-use')]),
    ).to.equal('port-in-use');
  });

  it('is blocked for any other failure, and nothing when no rule failed', () => {
    expect(blockedChipReason([failedMapping('no-entitlement')])).to.equal('blocked');
    expect(blockedChipReason([mappedMapping(0, NatPmpProto.udp, 55484)])).to.equal(undefined);
  });
});
