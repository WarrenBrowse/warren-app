import { IPv4Range, IPv6Range } from '../../../lib/ip';

export type LanNetworkError = 'invalid' | 'too-broad';

// The daemon refuses anything wider than the widest built-in ranges (10.0.0.0/8 and fc00::/7),
// since sharing it would carry a large part of the Internet around the tunnel.
const MIN_IPV4_PREFIX = 8;
const MIN_IPV6_PREFIX = 7;

// Returns why `input` cannot be shared by local network sharing, or undefined when it can.
export function checkLanNetwork(input: string): LanNetworkError | undefined {
  const network = input.trim();
  try {
    return IPv4Range.fromString(network).prefixSize < MIN_IPV4_PREFIX ? 'too-broad' : undefined;
  } catch {
    // Not IPv4, try IPv6.
  }
  try {
    return IPv6Range.fromString(network).prefixSize < MIN_IPV6_PREFIX ? 'too-broad' : undefined;
  } catch {
    return 'invalid';
  }
}
