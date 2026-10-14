import { useCallback, useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import {
  WithdrawablePurchase,
  WithdrawalAcknowledgement,
  WithdrawalFailure,
} from '../../../../shared/withdrawal';
import { useAppContext } from '../../../context';
import { Button, Flex, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { colors } from '../../../lib/foundations';
import SimpleInput from '../../SimpleInput';

const Card = styled.div`
  background-color: ${colors.blue10};
  border-radius: 16px;
  padding: 16px;
`;

function failureText(reason: WithdrawalFailure): string {
  switch (reason) {
    case 'not-proven':
      // TRANSLATORS: Shown when the server refuses a withdrawal because the
      // TRANSLATORS: purchase was not redeemed by this account.
      return messages.pgettext('account-view', 'This purchase was not credited to this account.');
    case 'already-filed':
      return messages.pgettext(
        'account-view',
        // TRANSLATORS: A withdrawal for this purchase already exists with another refund address.
        'A withdrawal with another refund address is already recorded: contact support.',
      );
    case 'invalid':
      // TRANSLATORS: The refund address typed for a crypto payment is not valid.
      return messages.pgettext('account-view', 'The refund address is not valid.');
    case 'no-wallet':
      // TRANSLATORS: Shown when no account is logged in on the device.
      return messages.pgettext('account-view', 'No account is logged in.');
    case 'unavailable':
      // TRANSLATORS: The withdrawal could not be recorded (network or server); retrying may work.
      return messages.pgettext(
        'account-view',
        'The withdrawal could not be recorded. Try again later.',
      );
  }
}

function acknowledgementText(ack: WithdrawalAcknowledgement): string {
  const recorded = ack.submittedAt
    ? sprintf(
        // TRANSLATORS: Proof of a recorded withdrawal.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(date)s - when the withdrawal was recorded
        // TRANSLATORS: %(reference)s - the reference of the declaration
        messages.pgettext(
          'account-view',
          'Withdrawal recorded on %(date)s, reference %(reference)s',
        ),
        { date: new Date(ack.submittedAt).toLocaleString(), reference: ack.reference },
      )
    : sprintf(
        // TRANSLATORS: Proof of a recorded withdrawal when its date is unknown.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(reference)s - the reference of the declaration
        messages.pgettext('account-view', 'Withdrawal recorded, reference %(reference)s'),
        { reference: ack.reference },
      );
  if (ack.refundAmountMinor === undefined || ack.currency === undefined) {
    return recorded;
  }
  const amount = new Intl.NumberFormat(undefined, {
    style: 'currency',
    currency: ack.currency,
  }).format(ack.refundAmountMinor / 100);
  const owed = sprintf(
    // TRANSLATORS: The amount that will be refunded after a withdrawal.
    // TRANSLATORS: Available placeholder:
    // TRANSLATORS: %(amount)s - the amount with its currency
    messages.pgettext('account-view', 'Refund owed: %(amount)s'),
    { amount },
  );
  return `${recorded}. ${owed}`;
}

function WithdrawablePurchaseRow(props: {
  purchase: WithdrawablePurchase;
  onDeclared: () => void;
}) {
  const { declareWithdrawal } = useAppContext();
  const { purchase, onDeclared } = props;
  const [open, setOpen] = useState(false);
  const [address, setAddress] = useState('');
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<WithdrawalFailure>();

  const onOpen = useCallback(() => setOpen(true), []);
  const onCancel = useCallback(() => setOpen(false), []);

  const onConfirm = useCallback(async () => {
    setBusy(true);
    setFailure(undefined);
    try {
      const result = await declareWithdrawal({ wpid: purchase.wpid, refundAddress: address });
      if (result.type === 'declared') {
        setOpen(false);
        onDeclared();
      } else {
        setFailure(result.reason);
      }
    } finally {
      setBusy(false);
    }
  }, [declareWithdrawal, purchase.wpid, address, onDeclared]);

  const title = sprintf(
    // TRANSLATORS: One purchase that can still be withdrawn from.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(date)s - when the purchase was credited
    // TRANSLATORS: %(deadline)s - last day of the 14-day withdrawal period
    messages.pgettext(
      'account-view',
      'Purchase of %(date)s, withdrawal possible until %(deadline)s',
    ),
    {
      date: new Date(purchase.redeemedMs).toLocaleDateString(),
      deadline: new Date(purchase.deadlineMs).toLocaleDateString(),
    },
  );

  return (
    <FlexColumn gap="tiny">
      <Text variant="bodySmall">{title}</Text>
      {purchase.acknowledgement ? (
        <Text variant="labelTiny" color="whiteAlpha60">
          {acknowledgementText(purchase.acknowledgement)}
        </Text>
      ) : open ? (
        <FlexColumn gap="tiny">
          <SimpleInput
            value={address}
            onChangeValue={setAddress}
            aria-label={
              // TRANSLATORS: Field for the address a crypto payment is refunded to.
              messages.pgettext('account-view', 'Refund address (crypto payment only)')
            }
            placeholder={messages.pgettext('account-view', 'Refund address (crypto payment only)')}
          />
          <Flex gap="small">
            <Button variant="destructive" disabled={busy} onClick={onConfirm} width="fit">
              <Button.Text>
                {
                  // TRANSLATORS: Button that records the withdrawal.
                  messages.pgettext('account-view', 'Confirm the withdrawal')
                }
              </Button.Text>
            </Button>
            <Button disabled={busy} onClick={onCancel} width="fit">
              <Button.Text>{messages.gettext('Cancel')}</Button.Text>
            </Button>
          </Flex>
          {failure !== undefined && (
            <Text variant="labelTiny" color="whiteAlpha60">
              {failureText(failure)}
            </Text>
          )}
        </FlexColumn>
      ) : (
        <Button onClick={onOpen} width="fit">
          <Button.Text>
            {
              // TRANSLATORS: Button that starts the 14-day withdrawal of a purchase.
              messages.pgettext('account-view', 'Withdraw')
            }
          </Button.Text>
        </Button>
      )}
    </FlexColumn>
  );
}

// The statutory 14-day withdrawal of a purchase made in this app: always
// reachable from the account view while the period runs (OUG 18/2026).
export function WithdrawalCard() {
  const { listWithdrawablePurchases } = useAppContext();
  const [purchases, setPurchases] = useState<WithdrawablePurchase[]>([]);

  const refresh = useCallback(() => {
    listWithdrawablePurchases()
      .then(setPurchases)
      .catch(() => setPurchases([]));
  }, [listWithdrawablePurchases]);

  useEffect(() => refresh(), [refresh]);

  if (purchases.length === 0) {
    return null;
  }
  return (
    <Card>
      <FlexColumn gap="small">
        <Text variant="labelTiny" color="whiteAlpha60">
          {
            // TRANSLATORS: Label of the block listing the purchases that can be withdrawn from.
            messages.pgettext('account-view', 'Withdrawal (14 days)')
          }
        </Text>
        {purchases.map((purchase) => (
          <WithdrawablePurchaseRow key={purchase.wpid} purchase={purchase} onDeclared={refresh} />
        ))}
      </FlexColumn>
    </Card>
  );
}
