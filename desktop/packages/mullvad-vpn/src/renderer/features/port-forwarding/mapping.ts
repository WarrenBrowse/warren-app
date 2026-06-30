import { NatPmpMapping, NatPmpRule } from '../../../shared/daemon-rpc-types';

/** The user-facing port of a rule (internal == suggested external in the
 * "same port on your device" model; fall back to either if one is 0). Used for
 * display, de-duplication and list keys - NOT for matching a live mapping (see
 * {@link mappingForRule}). */
export function rulePort(rule: NatPmpRule): number {
  return rule.internalPort !== 0 ? rule.internalPort : rule.suggestedExternalPort;
}

/** Find the live mapping matching a rule by its exit-side identity
 * `(internalPort, protocol)` - the exact key the daemon stores mappings under.
 *
 * Match on the raw `internalPort`, NOT `rulePort()`: an auto rule has
 * `internalPort === 0` (with the granted public port carried in
 * `externalPort`), so the daemon reports the mapping with `internalPort: 0`.
 * Matching on `rulePort()` (which falls back to `suggestedExternalPort`) would
 * look for `internalPort === <suggested>` and never find the live mapping,
 * stranding the row on "pending…" forever even though the exit mapped the port
 * successfully. */
export function mappingForRule(
  mappings: NatPmpMapping[],
  rule: NatPmpRule,
): NatPmpMapping | undefined {
  return mappings.find((m) => m.protocol === rule.protocol && m.internalPort === rule.internalPort);
}
