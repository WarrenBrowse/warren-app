import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import {
  NatPmpErrorReason,
  NatPmpMapping,
  NatPmpProto,
  NatPmpRule,
} from '../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../shared/gettext';
import { RoutePath } from '../../../../../shared/routes';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { spacings } from '../../../../lib/foundations';
import { useHistory } from '../../../../lib/history';
import {
  formatCountdown,
  NATPMP_MAX_RULES,
  useNatPmpPortBlock,
  usePortForwarding,
} from '../../hooks';
import { mappingForRule, rulePort } from '../../mapping';

const StyledRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  width: '100%',
  gap: '12px',
});

const StyledInput = styled.input<{ $disabled: boolean; $invalid: boolean }>(
  ({ $disabled, $invalid }) => ({
    background: 'transparent',
    border: 'none',
    borderBottom: `1px solid ${$invalid ? '#e34c45' : 'rgba(255,255,255,0.4)'}`,
    color: $disabled ? 'rgba(255,255,255,0.4)' : 'white',
    fontFamily: 'inherit',
    fontSize: '14px',
    padding: '4px 0',
    textAlign: 'right',
    width: '7ch',
    cursor: $disabled ? 'not-allowed' : 'text',
    '&:focus': {
      outline: 'none',
      borderBottomColor: $invalid ? '#e34c45' : 'white',
    },
  }),
);

const StyledSelect = styled.select<{ $disabled: boolean }>(({ $disabled }) => ({
  background: 'transparent',
  border: 'none',
  borderBottom: '1px solid rgba(255,255,255,0.4)',
  color: $disabled ? 'rgba(255,255,255,0.4)' : 'white',
  fontFamily: 'inherit',
  fontSize: '14px',
  padding: '4px 0',
  cursor: $disabled ? 'not-allowed' : 'pointer',
  '&:focus': {
    outline: 'none',
    borderBottomColor: 'white',
  },
  '& option': {
    color: 'black',
  },
}));

const StyledRemoveButton = styled.button<{ $disabled: boolean }>(({ $disabled }) => ({
  background: 'transparent',
  border: 'none',
  color: $disabled ? 'rgba(255,255,255,0.3)' : 'rgba(255,255,255,0.7)',
  cursor: $disabled ? 'not-allowed' : 'pointer',
  fontSize: '18px',
  lineHeight: 1,
  padding: '0 4px',
  '&:hover': {
    color: $disabled ? 'rgba(255,255,255,0.3)' : 'white',
  },
}));

const StyledAddButton = styled.button<{ $disabled: boolean }>(({ $disabled }) => ({
  background: 'transparent',
  border: 'none',
  color: $disabled ? 'rgba(255,255,255,0.3)' : '#44ad4d',
  cursor: $disabled ? 'not-allowed' : 'pointer',
  fontFamily: 'inherit',
  fontSize: '14px',
  padding: '4px 0',
  textAlign: 'left',
  '&:hover': {
    textDecoration: $disabled ? 'none' : 'underline',
  },
}));

const StyledStatus = styled.div({
  minWidth: '11ch',
  textAlign: 'right',
});

const StyledConflictActions = styled.div({
  display: 'flex',
  flexWrap: 'wrap',
  gap: '16px',
});

const StyledLinkButton = styled.button<{ $disabled: boolean }>(({ $disabled }) => ({
  background: 'transparent',
  border: 'none',
  color: $disabled ? 'rgba(255,255,255,0.3)' : '#44ad4d',
  cursor: $disabled ? 'not-allowed' : 'pointer',
  fontFamily: 'inherit',
  fontSize: '13px',
  padding: 0,
  textAlign: 'left',
  '&:hover': {
    textDecoration: $disabled ? 'none' : 'underline',
  },
}));

// Must match the exit allocator's range (warren-config
// NATPMP_EXTERNAL_PORT_MIN/MAX). A port outside this range can never be
// honoured by the exit, so we reject it client-side.
const MIN_PORT = 49152;
const MAX_PORT = 65535;

/** First port in range not already used by another rule of the same
 * protocol - a sensible, valid default for a freshly-added row. */
function nextFreePort(rules: NatPmpRule[], protocol: NatPmpProto): number {
  const used = new Set(rules.filter((r) => r.protocol === protocol).map((r) => rulePort(r)));
  for (let p = MIN_PORT; p <= MAX_PORT; p++) {
    if (!used.has(p)) {
      return p;
    }
  }
  return MIN_PORT;
}

/**
 * Multi-port NAT-PMP editor: a list of port-forward rules (protocol +
 * port), each with its live status, plus an "add a port" affordance and
 * the shared rate-limit countdown. Rendered below the toggle in
 * `PortForwardingSettingsView` when port forwarding is enabled.
 *
 * - Each rule's port is used as BOTH the internal and the suggested
 *   external port ("same port on your device"): the public port the user
 *   picks is the port their app must bind locally.
 * - Up to {@link NATPMP_MAX_RULES} rules (the exit quota). "Add a port"
 *   disables at the cap.
 * - Editing/adding applies immediately (live reconfig - no reconnect).
 * - While the exit rate-limits this client (shared budget), every control
 *   disables with a precise mm:ss countdown so the client never trips the
 *   ban itself.
 */
export function PortForwardingAdvanced() {
  const { rules, mappings, addRule, updateRule, removeRule } = usePortForwarding();
  const block = useNatPmpPortBlock();
  const history = useHistory();
  const controlsDisabled = block.blocked;

  const handleAdd = React.useCallback(() => {
    if (rules.length >= NATPMP_MAX_RULES || controlsDisabled) {
      return;
    }
    const port = nextFreePort(rules, NatPmpProto.udp);
    void addRule({
      protocol: NatPmpProto.udp,
      suggestedExternalPort: port,
      internalPort: port,
    });
  }, [rules, controlsDisabled, addRule]);

  const isDuplicate = React.useCallback(
    (port: number, protocol: NatPmpProto, exceptIndex: number) =>
      rules.some((r, i) => i !== exceptIndex && r.protocol === protocol && rulePort(r) === port),
    [rules],
  );

  // Stable, index-keyed handlers so the row JSX passes a function
  // reference rather than a fresh closure (react/jsx-no-bind).
  const handleRuleProtocol = React.useCallback(
    (index: number, protocol: NatPmpProto) => {
      const port = rulePort(rules[index]);
      void updateRule(index, { protocol, suggestedExternalPort: port, internalPort: port });
    },
    [rules, updateRule],
  );

  const handleRulePort = React.useCallback(
    (index: number, port: number) => {
      const { protocol } = rules[index];
      void updateRule(index, { protocol, suggestedExternalPort: port, internalPort: port });
    },
    [rules, updateRule],
  );

  const handleRuleRemove = React.useCallback(
    (index: number) => {
      void removeRule(index);
    },
    [removeRule],
  );

  // Conflict resolution "let the exit pick a free port": keep the local
  // (internal) port the app listens on, but drop the suggested external
  // port to 0 so the exit allocates any free one. Decouples internal from
  // external for this rule; the granted public port shows in the row
  // status. The remap is live (no reconnect).
  const handleResolveAuto = React.useCallback(
    (index: number) => {
      const { protocol } = rules[index];
      const port = rulePort(rules[index]);
      void updateRule(index, { protocol, suggestedExternalPort: 0, internalPort: port });
    },
    [rules, updateRule],
  );

  // Conflict resolution "choose another exit": the followed port was taken
  // on this exit; send the user to the location picker to switch.
  const handleChangeExit = React.useCallback(() => {
    history.push(RoutePath.selectLocation);
  }, [history]);

  return (
    <FlexColumn gap="small">
      <SettingsListItem anchorId="port-forwarding-advanced">
        <SettingsListItem.Item>
          <FlexColumn gap="medium" style={{ width: '100%', paddingRight: spacings.medium }}>
            {rules.length === 0 ? (
              <Text variant="bodySmall" color="whiteAlpha60">
                {messages.pgettext(
                  'port-forwarding-view',
                  'No port yet. Add one to open it on the exit.',
                )}
              </Text>
            ) : (
              rules.map((rule, index) => (
                <PortRuleRow
                  key={`${rule.protocol}-${rulePort(rule)}-${index}`}
                  rule={rule}
                  index={index}
                  mapping={mappingForRule(mappings, rule)}
                  disabled={controlsDisabled}
                  isDuplicate={isDuplicate}
                  onChangeProtocol={handleRuleProtocol}
                  onChangePort={handleRulePort}
                  onRemove={handleRuleRemove}
                  onResolveAuto={handleResolveAuto}
                  onChangeExit={handleChangeExit}
                />
              ))
            )}
          </FlexColumn>
        </SettingsListItem.Item>
      </SettingsListItem>

      <StyledAddButton
        type="button"
        $disabled={controlsDisabled || rules.length >= NATPMP_MAX_RULES}
        disabled={controlsDisabled || rules.length >= NATPMP_MAX_RULES}
        onClick={handleAdd}>
        {rules.length >= NATPMP_MAX_RULES
          ? sprintf(messages.pgettext('port-forwarding-view', 'Maximum of %(max)d ports reached'), {
              max: NATPMP_MAX_RULES,
            })
          : messages.pgettext('port-forwarding-view', '+ Add a port')}
      </StyledAddButton>

      {/* Single SHARED rate-limit warning for the whole view: the budget
          is per-client, so one countdown governs every row. budget-exhausted
          / rate-limited disable the controls with a precise mm:ss countdown
          to when the next change is allowed - the client therefore never
          lets the user trip the exit's rate-limit. last-chance keeps the
          controls enabled but warns that one more change triggers a block.
          Both self-clear from the clock (see useNatPmpPortBlock). */}
      {block.reason === 'budget-exhausted' || block.reason === 'rate-limited' ? (
        <Text variant="labelTiny" color="yellow">
          {sprintf(
            messages.pgettext(
              'port-forwarding-view',
              'Too many port changes. You can change ports again in %(countdown)s.',
            ),
            { countdown: formatCountdown(block.remainingSecs) },
          )}
        </Text>
      ) : block.reason === 'last-chance' ? (
        <Text variant="labelTiny" color="yellow">
          {messages.pgettext(
            'port-forwarding-view',
            'Last port change before a temporary block. Wait a moment before changing it again.',
          )}
        </Text>
      ) : null}

      <Text variant="labelTiny" color="whiteAlpha60">
        {messages.pgettext(
          'port-forwarding-view',
          'Each port you add is opened on the exit and forwarded to the same port on your device. Changes apply immediately, no reconnection needed.',
        )}
      </Text>
    </FlexColumn>
  );
}

interface PortRuleRowProps {
  rule: NatPmpRule;
  index: number;
  mapping: NatPmpMapping | undefined;
  disabled: boolean;
  isDuplicate: (port: number, protocol: NatPmpProto, exceptIndex: number) => boolean;
  onChangeProtocol: (index: number, protocol: NatPmpProto) => void;
  onChangePort: (index: number, port: number) => void;
  onRemove: (index: number) => void;
  onResolveAuto: (index: number) => void;
  onChangeExit: () => void;
}

function PortRuleRow({
  rule,
  index,
  mapping,
  disabled,
  isDuplicate,
  onChangeProtocol,
  onChangePort,
  onRemove,
  onResolveAuto,
  onChangeExit,
}: PortRuleRowProps) {
  const committedPort = rulePort(rule);
  const [portDraft, setPortDraft] = React.useState<string>(
    committedPort === 0 ? '' : String(committedPort),
  );
  const [error, setError] = React.useState<string | null>(null);

  // Re-sync when the persisted rule changes underneath us (e.g. a fresh
  // settings snapshot), unless the user is mid-edit on the same value.
  React.useEffect(() => {
    const next = committedPort === 0 ? '' : String(committedPort);
    if (Number(portDraft) === committedPort) {
      return;
    }
    setPortDraft(next);
    // eslint-disable-next-line react-compiler/react-compiler
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [committedPort]);

  const commitPort = React.useCallback(() => {
    const parsed = Number(portDraft.trim());
    if (!Number.isInteger(parsed) || parsed < MIN_PORT || parsed > MAX_PORT) {
      setError(messages.pgettext('port-forwarding-view', 'Port must be between 49152 and 65535.'));
      return;
    }
    if (isDuplicate(parsed, rule.protocol, index)) {
      setError(messages.pgettext('port-forwarding-view', 'This port is already in your list.'));
      return;
    }
    setError(null);
    if (parsed !== committedPort) {
      onChangePort(index, parsed);
    }
  }, [portDraft, rule.protocol, index, isDuplicate, committedPort, onChangePort]);

  const handlePortChange = React.useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setPortDraft(e.target.value.replace(/[^0-9]/g, '').slice(0, 5));
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
      onChangeProtocol(index, e.target.value === 'tcp' ? NatPmpProto.tcp : NatPmpProto.udp);
    },
    [onChangeProtocol, index],
  );

  const handleRemoveClick = React.useCallback(() => {
    onRemove(index);
  }, [onRemove, index]);

  const handleResolveAutoClick = React.useCallback(() => {
    onResolveAuto(index);
  }, [onResolveAuto, index]);

  // A followed port can be taken by another client on a new exit: the
  // exit rejects the suggested port (strict honour-or-error). Offer the
  // user a way out instead of leaving the mapping silently dead. Editing
  // the port field above is the third option (pick a specific new port).
  const portConflict =
    mapping?.status.state === 'failed' && mapping.status.errorReason === 'suggested-port-in-use';

  return (
    <FlexColumn gap="tiny">
      <StyledRow>
        <StyledSelect
          $disabled={disabled}
          disabled={disabled}
          value={rule.protocol}
          onChange={handleProtocolChange}
          aria-label={messages.pgettext('port-forwarding-view', 'Protocol')}>
          <option value={NatPmpProto.udp}>UDP</option>
          <option value={NatPmpProto.tcp}>TCP</option>
        </StyledSelect>
        <StyledInput
          $disabled={disabled}
          $invalid={error !== null}
          disabled={disabled}
          type="text"
          inputMode="numeric"
          pattern="[0-9]*"
          value={portDraft}
          onChange={handlePortChange}
          onBlur={commitPort}
          onKeyDown={handlePortKeyDown}
          placeholder={messages.pgettext('port-forwarding-view', 'port')}
          aria-label={messages.pgettext('port-forwarding-view', 'Port')}
        />
        <StyledStatus>
          <RuleStatus mapping={mapping} />
        </StyledStatus>
        <StyledRemoveButton
          type="button"
          $disabled={disabled}
          disabled={disabled}
          onClick={handleRemoveClick}
          aria-label={messages.pgettext('port-forwarding-view', 'Remove port')}
          title={messages.pgettext('port-forwarding-view', 'Remove port')}>
          ✕
        </StyledRemoveButton>
      </StyledRow>
      {error !== null ? (
        <Text variant="labelTiny" color="red">
          {error}
        </Text>
      ) : null}
      {portConflict ? (
        <FlexColumn gap="tiny">
          <Text variant="labelTiny" color="whiteAlpha60">
            {messages.pgettext(
              'port-forwarding-view',
              'This public port is already in use on this exit. Keep your port by switching exit, let the exit assign a free public port, or type another port above.',
            )}
          </Text>
          <StyledConflictActions>
            <StyledLinkButton
              type="button"
              $disabled={disabled}
              disabled={disabled}
              onClick={handleResolveAutoClick}>
              {messages.pgettext('port-forwarding-view', 'Assign a free port')}
            </StyledLinkButton>
            <StyledLinkButton type="button" $disabled={false} onClick={onChangeExit}>
              {messages.pgettext('port-forwarding-view', 'Choose another exit')}
            </StyledLinkButton>
          </StyledConflictActions>
        </FlexColumn>
      ) : null}
    </FlexColumn>
  );
}

/** Per-rule live status, shown inline on its row. */
function RuleStatus({ mapping }: { mapping: NatPmpMapping | undefined }) {
  if (!mapping) {
    return (
      <Text variant="labelTiny" color="whiteAlpha60">
        {messages.pgettext('port-forwarding-view', 'pending…')}
      </Text>
    );
  }
  const status = mapping.status;
  switch (status.state) {
    case 'mapped':
      return (
        <Text variant="labelTiny" color="green">
          {sprintf(messages.pgettext('port-forwarding-view', 'open: %(port)d'), {
            port: status.externalPort,
          })}
        </Text>
      );
    case 'rate-limited':
      return (
        <Text variant="labelTiny" color="whiteAlpha60">
          {messages.pgettext('port-forwarding-view', 'applying…')}
        </Text>
      );
    case 'failed':
      return (
        <Text variant="labelTiny" color="red">
          {natPmpShortFailure(status.errorReason)}
        </Text>
      );
    case 'disabled':
      return (
        <Text variant="labelTiny" color="whiteAlpha60">
          {messages.pgettext('port-forwarding-view', 'disabled')}
        </Text>
      );
    case 'requesting':
    default:
      return (
        <Text variant="labelTiny" color="whiteAlpha60">
          {messages.pgettext('port-forwarding-view', 'requesting…')}
        </Text>
      );
  }
}

/** Short, inline failure label keyed on the structured reason. */
function natPmpShortFailure(reason: NatPmpErrorReason): string {
  switch (reason) {
    case 'suggested-port-in-use':
      return messages.pgettext('port-forwarding-view', 'port in use');
    case 'out-of-resources':
      return messages.pgettext('port-forwarding-view', 'no port available');
    case 'not-authorized':
      return messages.pgettext('port-forwarding-view', 'not allowed');
    case 'unknown':
    default:
      return messages.pgettext('port-forwarding-view', 'failed');
  }
}
