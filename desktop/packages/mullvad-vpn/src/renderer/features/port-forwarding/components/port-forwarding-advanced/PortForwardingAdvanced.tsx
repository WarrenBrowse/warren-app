import React from 'react';
import styled from 'styled-components';

import { NatPmpProto } from '../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../shared/gettext';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { spacings } from '../../../../lib/foundations';
import { usePortForwarding } from '../../hooks';

// Plain styled inputs (no design-system `TextField` shell here) match
// the pattern used by `WarrenMultiHopCountryPickers`: a SettingsListItem
// row containing a label + a minimal input. Keeps the visual weight
// proportional to the row above (the on/off toggle) without dragging
// in the full TextField hook plumbing for two simple values.

const StyledRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  width: '100%',
  gap: '12px',
});

const StyledInput = styled.input<{ $disabled: boolean }>(({ $disabled }) => ({
  background: 'transparent',
  border: 'none',
  borderBottom: '1px solid rgba(255,255,255,0.4)',
  color: $disabled ? 'rgba(255,255,255,0.4)' : 'white',
  fontFamily: 'inherit',
  fontSize: '14px',
  padding: '4px 0',
  textAlign: 'right',
  width: '8ch',
  cursor: $disabled ? 'not-allowed' : 'text',
  '&:focus': {
    outline: 'none',
    borderBottomColor: 'white',
  },
}));

const StyledSelect = styled.select<{ $disabled: boolean }>(({ $disabled }) => ({
  background: 'transparent',
  border: 'none',
  borderBottom: '1px solid rgba(255,255,255,0.4)',
  color: $disabled ? 'rgba(255,255,255,0.4)' : 'white',
  fontFamily: 'inherit',
  fontSize: '14px',
  padding: '4px 0',
  cursor: $disabled ? 'not-allowed' : 'pointer',
  // The native picker arrow is platform-dependent and looks fine on
  // both macOS and Windows. We only ensure the dropdown items render
  // with the system's dark/light scheme, not the chrome's translucent
  // dark blue — readable contrast is more important than visual
  // continuity.
  '&:focus': {
    outline: 'none',
    borderBottomColor: 'white',
  },
  '& option': {
    color: 'black',
  },
}));

// Must match the exit allocator's range (warren-config
// NATPMP_EXTERNAL_PORT_MIN/MAX). A preferred port outside this range
// can never be honoured by the exit — it would silently fall back to
// a random port — so we reject it client-side instead of misleading
// the user.
const MIN_PORT = 49152;
const MAX_PORT = 65535;

// Locked rows when the daemon already holds a mapping (state ===
// 'mapped' or 'requesting'). Changing `protocol` or
// `suggestedExternalPort` while the live mapping is established does
// NOT renegotiate the mapping — the daemon's NAT-PMP manager is
// spawned at tunnel start and reads its config once. Locking the
// inputs in those states avoids the misleading UX where the user
/**
 * Advanced port-forwarding controls: TCP/UDP protocol + suggested
 * port. Rendered below the toggle in `PortForwardingSettingsView`.
 *
 * Behaviour:
 * - Suggested port defaults to `0` ("server picks"). Any value in
 *   `[49152, 65535]` is sent to the daemon; the exit honours it when
 *   the port is free, or allocates a different one if already taken
 *   (the granted port is shown live in `PortForwardingStatus`).
 * - Protocol defaults to `udp` (covers BitTorrent + most P2P + UDP
 *   games). `tcp` is required by Minecraft, FTP, IRC servers.
 * - **Live reconfig (M5.D.x)**: editing the protocol or port applies
 *   the change immediately — the daemon's in-tunnel NAT-PMP
 *   controller releases the current mapping and allocates a new one
 *   without a tunnel reconnect. The inputs are therefore always
 *   editable; the change is committed on protocol-select / port-blur
 *   and the new state surfaces in `PortForwardingStatus`
 *   ("requesting…" → "active" with the new port).
 */
export function PortForwardingAdvanced() {
  const { settings, setProtocol, setSuggestedExternalPort } = usePortForwarding();

  // Local state for the port input so the user can clear / retype
  // without the redux value clobbering keystrokes mid-edit. We push
  // on blur (or Enter) so transient invalid intermediate states
  // (e.g., "5" while typing "50000") do not spam the daemon.
  const [portDraft, setPortDraft] = React.useState<string>(
    settings.suggestedExternalPort === 0 ? '' : String(settings.suggestedExternalPort),
  );
  const [invalid, setInvalid] = React.useState(false);

  // Re-sync when the redux value changes (e.g., the daemon pushes a
  // fresh settings snapshot after a settings.json reload). The
  // comparison guards against the typical case where the local
  // draft is the source of truth in mid-edit.
  React.useEffect(() => {
    const fromState =
      settings.suggestedExternalPort === 0 ? '' : String(settings.suggestedExternalPort);
    if (portDraft === '' && settings.suggestedExternalPort === 0) {
      return;
    }
    if (Number(portDraft) === settings.suggestedExternalPort) {
      return;
    }
    setPortDraft(fromState);
    // eslint-disable-next-line react-compiler/react-compiler
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.suggestedExternalPort]);

  const commitPort = React.useCallback(() => {
    const trimmed = portDraft.trim();
    if (trimmed === '') {
      setInvalid(false);
      void setSuggestedExternalPort(0);
      return;
    }
    const parsed = Number(trimmed);
    if (!Number.isInteger(parsed) || parsed < MIN_PORT || parsed > MAX_PORT) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    void setSuggestedExternalPort(parsed);
  }, [portDraft, setSuggestedExternalPort]);

  const handlePortChange = React.useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const next = e.target.value.replace(/[^0-9]/g, '').slice(0, 5);
    setPortDraft(next);
  }, []);

  const handlePortKeyDown = React.useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        commitPort();
        (e.target as HTMLInputElement).blur();
      }
    },
    [commitPort],
  );

  const handleProtocolChange = React.useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const proto = e.target.value === 'tcp' ? NatPmpProto.tcp : NatPmpProto.udp;
      void setProtocol(proto);
    },
    [setProtocol],
  );

  return (
    <FlexColumn gap="small">
      <SettingsListItem anchorId="port-forwarding-advanced">
        <SettingsListItem.Item>
          {/*
            `ListItemItem` applies a left `padding-left: spacings.medium`
            on its first child via `useIndent()` but does NOT mirror it
            on the right side. Without an explicit `paddingRight`, the
            native `<select>` arrow and the `<input>` text get visually
            clipped against the container's right edge (observed on
            macOS 14, screenshot 2026-05-28). Add a symmetric 16 px
            so the controls breathe.
          */}
          <FlexColumn gap="medium" style={{ width: '100%', paddingRight: spacings.medium }}>
            <StyledRow>
              <Text variant="bodySmall">
                {messages.pgettext('port-forwarding-view', 'Protocol')}
              </Text>
              <StyledSelect
                $disabled={false}
                value={settings.protocol}
                onChange={handleProtocolChange}
                aria-label={messages.pgettext('port-forwarding-view', 'Protocol')}>
                <option value={NatPmpProto.udp}>UDP</option>
                <option value={NatPmpProto.tcp}>TCP</option>
              </StyledSelect>
            </StyledRow>
            <StyledRow>
              <Text variant="bodySmall">
                {messages.pgettext('port-forwarding-view', 'Preferred port')}
              </Text>
              <StyledInput
                $disabled={false}
                type="text"
                inputMode="numeric"
                pattern="[0-9]*"
                value={portDraft}
                onChange={handlePortChange}
                onBlur={commitPort}
                onKeyDown={handlePortKeyDown}
                placeholder={messages.pgettext('port-forwarding-view', 'auto')}
                aria-label={messages.pgettext('port-forwarding-view', 'Preferred port')}
                style={invalid ? { borderBottomColor: '#e34c45' } : undefined}
              />
            </StyledRow>
          </FlexColumn>
        </SettingsListItem.Item>
      </SettingsListItem>
      {invalid ? (
        <Text variant="labelTiny" color="red">
          {messages.pgettext(
            'port-forwarding-view',
            'Port must be between 49152 and 65535, or empty for auto.',
          )}
        </Text>
      ) : null}
      <Text variant="labelTiny" color="whiteAlpha60">
        {messages.pgettext(
          'port-forwarding-view',
          'The server may assign a different port if the preferred one is already taken. Changes apply immediately, no reconnection needed.',
        )}
      </Text>
    </FlexColumn>
  );
}
