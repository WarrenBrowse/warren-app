import { describe, expect, it } from 'vitest';

import { parseSeenPorts, serializeSeenPorts } from '../../src/main/port-forwarding-seen';
import { NatPmpProto, NatPmpStatus } from '../../src/shared/daemon-rpc-types';

const granted: NatPmpStatus = {
  mappings: [
    {
      internalPort: 60504,
      protocol: NatPmpProto.both,
      status: {
        state: 'mapped',
        externalPort: 60504,
        lifetimeGrantedSecs: 3600,
        windowResetSecs: 0,
      },
    },
  ],
};

describe('the ports the app last saw, kept across GUI restarts', () => {
  it('reads back what it wrote, as the rule and the port only', () => {
    const read = parseSeenPorts(serializeSeenPorts(granted));

    expect(read?.mappings).toHaveLength(1);
    expect(read?.mappings[0].internalPort).toBe(60504);
    expect(read?.mappings[0].protocol).toBe(NatPmpProto.both);
    expect(read?.mappings[0].status).toMatchObject({ state: 'mapped', externalPort: 60504 });
  });

  it('keeps no rule that holds no port', () => {
    const asking: NatPmpStatus = {
      mappings: [
        { internalPort: 60504, protocol: NatPmpProto.both, status: { state: 'requesting' } },
      ],
    };

    expect(parseSeenPorts(serializeSeenPorts(asking))?.mappings).toEqual([]);
  });

  // A damaged file must cost one extra notification, never a crash of the
  // main process at startup.
  it('reads a damaged or foreign file as nothing seen', () => {
    expect(parseSeenPorts('')).toBeUndefined();
    expect(parseSeenPorts('{not json')).toBeUndefined();
    expect(parseSeenPorts('{"ports":[{"internalPort":"x"}]}')).toBeUndefined();
    expect(
      parseSeenPorts('{"ports":[{"internalPort":1,"protocol":"sctp","port":2}]}'),
    ).toBeUndefined();
  });
});
