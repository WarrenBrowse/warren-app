import { describe, expect, it } from 'vitest';

import { mappingForRule, rulePort } from '../../src/renderer/features/port-forwarding/mapping';
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
