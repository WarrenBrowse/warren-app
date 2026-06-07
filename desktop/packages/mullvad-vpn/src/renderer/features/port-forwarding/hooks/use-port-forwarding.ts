import React from 'react';

import {
  effectiveNatPmpRules,
  NatPmpProto,
  NatPmpRule,
  NatPmpSettings,
} from '../../../../shared/daemon-rpc-types';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

// Exit-enforced per-client quota (`warren_config::NATPMP_QUOTA_PER_CLIENT_IP`).
// The UI caps the rule list at this and disables "add a port" once reached
// so the client never asks the exit for more than it will grant.
export const NATPMP_MAX_RULES = 5;

/**
 * Hook exposing the NAT-PMP port-forwarding settings + live status to
 * the port-forwarding view + its sub-components.
 *
 * Multi-port model: the user maintains a list of {@link NatPmpRule}s (up
 * to {@link NATPMP_MAX_RULES}). Each rule's identity exit-side is
 * `(internalPort, protocol)`, so the UI sets `internalPort ===
 * suggestedExternalPort` (the single port number the user picks). The
 * daemon keeps one mapping per rule; the live `mappings` array carries
 * per-rule status.
 *
 * Writes go through `setNatPmpSettings` (pushed live — the daemon's
 * in-tunnel controller reconciles the running mappings without a tunnel
 * reconnect). New writes populate `rules` and zero the legacy
 * single-port fields.
 */
export function usePortForwarding() {
  const settings = useSelector((state) => state.settings.warrenNatPmp);
  const status = useSelector((state) => state.settings.natPmpStatus) ?? { mappings: [] };
  // Real wall-clock instant the live status arrived (see
  // useNatPmpPortBlock: the rate-limit countdown anchors to this, not to
  // component mount time, so a stale snapshot self-expires).
  const statusReceivedAt = useSelector((state) => state.settings.natPmpStatusReceivedAt);
  const { setNatPmpSettings } = useAppContext();

  const rules = React.useMemo(() => effectiveNatPmpRules(settings), [settings]);

  const pushUpdate = React.useCallback(
    async (patch: Partial<NatPmpSettings>) => {
      try {
        await setNatPmpSettings({ ...settings, ...patch });
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set NAT-PMP settings', message);
      }
    },
    [settings, setNatPmpSettings],
  );

  const setEnabled = React.useCallback(
    async (enabled: boolean) => {
      await pushUpdate({ enabled });
    },
    [pushUpdate],
  );

  const setLifetimeSecs = React.useCallback(
    async (lifetimeSecs: number) => {
      await pushUpdate({ lifetimeSecs });
    },
    [pushUpdate],
  );

  // Write the rule list as the source of truth and zero the legacy
  // single-port fields so a future read prefers `rules`.
  const setRules = React.useCallback(
    async (newRules: NatPmpRule[]) => {
      await pushUpdate({
        rules: newRules,
        protocol: NatPmpProto.udp,
        suggestedExternalPort: 0,
        internalPort: 0,
      });
    },
    [pushUpdate],
  );

  const addRule = React.useCallback(
    async (rule: NatPmpRule) => {
      if (rules.length >= NATPMP_MAX_RULES) {
        return;
      }
      await setRules([...rules, rule]);
    },
    [rules, setRules],
  );

  const updateRule = React.useCallback(
    async (index: number, rule: NatPmpRule) => {
      await setRules(rules.map((r, i) => (i === index ? rule : r)));
    },
    [rules, setRules],
  );

  const removeRule = React.useCallback(
    async (index: number) => {
      await setRules(rules.filter((_, i) => i !== index));
    },
    [rules, setRules],
  );

  return {
    settings,
    rules,
    status,
    mappings: status.mappings,
    statusReceivedAt,
    setEnabled,
    setLifetimeSecs,
    setRules,
    addRule,
    updateRule,
    removeRule,
  };
}
