import { useCallback, useState } from 'react';
import styled from 'styled-components';

import { WarrenPubkeyMismatch } from '../../../../shared/daemon-rpc-types';
import { messages } from '../../../../shared/gettext';
import { ModalAlert, ModalAlertType } from '../../../components/Modal';
import { useAppContext } from '../../../context';
import { Button } from '../../../lib/components';
import { colors } from '../../../lib/foundations';
import { useSelector } from '../../../redux/store';
import { truncatePubkeyHex } from '../lib/truncate-pubkey';

// Session A.4 TOFU pubkey-pinning mismatch surface. Mounts a modal
// overlay when the daemon-side verify hook refuses a connect because
// the Ed25519 pubkey served for a known `exit_id` differs from the
// locally pinned value. The user picks one of three actions:
//
//   * "Trust new key" -> gRPC TrustNewExitKey + reconnect attempt
//   * "Reject (disconnect)" -> clear pending mismatch, stay
//     disconnected
//   * "Report to Warren" -> POST /v1/incidents/pubkey-mismatch
//     (best-effort) + clear pending + stay disconnected
//
// The modal renders nothing in the steady state
// (`pubkeyMismatchPending === null`).
export function WarrenPubKeyWarning() {
  const pending = useSelector((state) => state.settings.warrenStatus?.pubkeyMismatchPending);
  const { trustNewExitKey, dismissPubkeyMismatch, reportPubkeyMismatch } = useAppContext();
  const [busy, setBusy] = useState(false);

  const handleTrust = useCallback(async () => {
    if (!pending) {
      return;
    }
    setBusy(true);
    try {
      await trustNewExitKey({
        exitIdHex: pending.exitIdHex,
        newPubkeyHex: pending.observedPubkeyHex,
      });
    } finally {
      setBusy(false);
    }
  }, [pending, trustNewExitKey]);

  const handleReject = useCallback(async () => {
    setBusy(true);
    try {
      await dismissPubkeyMismatch();
    } finally {
      setBusy(false);
    }
  }, [dismissPubkeyMismatch]);

  const handleReport = useCallback(async () => {
    if (!pending) {
      return;
    }
    setBusy(true);
    try {
      await reportPubkeyMismatch(pending);
    } finally {
      setBusy(false);
    }
  }, [pending, reportPubkeyMismatch]);

  const isOpen = pending !== null && pending !== undefined;

  return (
    <ModalAlert
      isOpen={isOpen}
      type={ModalAlertType.warning}
      iconColor={colors.red}
      title={messages.pgettext('warren-pubkey-warning', 'Server identity changed')}
      message={[
        messages.pgettext(
          'warren-pubkey-warning',
          'The Warren exit server you previously trusted now presents a different cryptographic identity.',
        ),
        messages.pgettext(
          'warren-pubkey-warning',
          'This usually means the operator rotated the key, but it can also indicate that the server has been replaced or compromised. Refuse if you did not expect a change.',
        ),
      ]}
      buttons={[
        <Button
          key="trust"
          variant="success"
          disabled={busy}
          onClick={handleTrust}
          aria-label={messages.pgettext('warren-pubkey-warning', 'Trust new key')}>
          <Button.Text>{messages.pgettext('warren-pubkey-warning', 'Trust new key')}</Button.Text>
        </Button>,
        <Button
          key="report"
          variant="primary"
          disabled={busy}
          onClick={handleReport}
          aria-label={messages.pgettext('warren-pubkey-warning', 'Report to Warren')}>
          <Button.Text>
            {messages.pgettext('warren-pubkey-warning', 'Report to Warren')}
          </Button.Text>
        </Button>,
        <Button
          key="reject"
          variant="destructive"
          disabled={busy}
          onClick={handleReject}
          aria-label={messages.pgettext('warren-pubkey-warning', 'Reject and stay disconnected')}>
          <Button.Text>
            {messages.pgettext('warren-pubkey-warning', 'Reject (disconnect)')}
          </Button.Text>
        </Button>,
      ]}
      close={handleReject}>
      {pending && <PubKeyWarningDetails pending={pending} />}
      <BusyStatusRegion role="status" aria-live="polite">
        {busy
          ? messages.pgettext('warren-pubkey-warning', 'Processing your choice, please wait.')
          : ''}
      </BusyStatusRegion>
    </ModalAlert>
  );
}

interface DetailsProps {
  pending: WarrenPubkeyMismatch;
}

const DetailsContainer = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: '6px',
  marginTop: '14px',
  padding: '10px 12px',
  backgroundColor: colors.blackAlpha50,
  borderRadius: '6px',
});

const DetailsRow = styled.div({
  display: 'flex',
  justifyContent: 'space-between',
  gap: '12px',
  fontSize: '11px',
  lineHeight: 1.5,
  color: colors.whiteAlpha80,
  fontFamily: 'monospace',
});

const DetailsLabel = styled.span({
  color: colors.whiteAlpha60,
  textTransform: 'uppercase',
  letterSpacing: '0.5px',
  fontFamily: 'inherit',
});

function PubKeyWarningDetails({ pending }: DetailsProps) {
  const detailsLabel = messages.pgettext('warren-pubkey-warning', 'Cryptographic mismatch details');
  return (
    <DetailsContainer role="group" aria-label={detailsLabel}>
      <DetailsRow>
        <DetailsLabel>{messages.pgettext('warren-pubkey-warning', 'Exit ID')}</DetailsLabel>
        <span title={pending.exitIdHex}>{truncatePubkeyHex(pending.exitIdHex)}</span>
      </DetailsRow>
      <DetailsRow>
        <DetailsLabel>
          {messages.pgettext('warren-pubkey-warning', 'Previously pinned key')}
        </DetailsLabel>
        <span title={pending.pinnedPubkeyHex}>{truncatePubkeyHex(pending.pinnedPubkeyHex)}</span>
      </DetailsRow>
      <DetailsRow>
        <DetailsLabel>
          {messages.pgettext('warren-pubkey-warning', 'Newly observed key')}
        </DetailsLabel>
        <span title={pending.observedPubkeyHex}>
          {truncatePubkeyHex(pending.observedPubkeyHex)}
        </span>
      </DetailsRow>
      {pending.countryCode && (
        <DetailsRow>
          <DetailsLabel>{messages.pgettext('warren-pubkey-warning', 'Location')}</DetailsLabel>
          <span>
            {pending.city ? `${pending.city}, ` : ''}
            {pending.countryCode.toUpperCase()}
          </span>
        </DetailsRow>
      )}
    </DetailsContainer>
  );
}

// Polite ARIA live region that announces the busy state to assistive
// tech while a CTA is in flight. The visible UI shows the disabled
// buttons via the `busy` flag in the parent; screen readers benefit
// from an explicit status update so the user knows the choice is
// being processed.
const BusyStatusRegion = styled.span({
  position: 'absolute',
  width: '1px',
  height: '1px',
  overflow: 'hidden',
  clip: 'rect(0 0 0 0)',
  whiteSpace: 'nowrap',
});
