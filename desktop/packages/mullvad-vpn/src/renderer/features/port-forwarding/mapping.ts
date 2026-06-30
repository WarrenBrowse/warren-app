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

/** The port to SHOW in a rule's input - the single source of truth that cannot
 * diverge from the live status displayed on its right.
 *
 * For an AUTO rule (`suggestedExternalPort === 0`: the user let the exit pick
 * the public port) that the exit has GRANTED, the applied port is that granted
 * external port, NOT the device port the rule started from. Without this, after
 * "assign a free port" the input keeps showing the old port P while the status
 * shows the granted port Q - a confusing divergence. For a manual rule (or one
 * that is not yet mapped) it is the rule's own committed port. */
export function appliedPort(rule: NatPmpRule, mapping: NatPmpMapping | undefined): number {
  if (rule.suggestedExternalPort === 0 && mapping?.status.state === 'mapped') {
    return mapping.status.externalPort;
  }
  return rulePort(rule);
}
