import React, { useCallback, useContext, useRef, useState } from 'react';
import { sprintf } from 'sprintf-js';

import { formatDate } from '../../shared/account-expiry';
import { VoucherResponse } from '../../shared/daemon-rpc-types';
import { formatRelativeDate } from '../../shared/date-helper';
import { messages } from '../../shared/gettext';
import { isWarrenPubKey, isWarrenVoucher } from '../../shared/utils';
import { useAppContext } from '../context';
import { Button, ButtonProps, Flex, Spinner } from '../lib/components';
import { IconBadge } from '../lib/icon-badge';
import { useSelector } from '../redux/store';
import { ModalAlert } from './Modal';
import {
  StyledEmptyResponse,
  StyledErrorResponse,
  StyledInput,
  StyledLabel,
  StyledProgressResponse,
  StyledTitle,
  StyledWarrenPubKeyInfo,
} from './RedeemVoucherStyles';

// Warren voucher format (Crockford-32, see `shared/utils.ts`):
// 16 raw chars displayed as `XXXX-XXXX-XXXX-XXXX` (19 chars). The
// `FormattableTextInput` is configured below to enforce the
// grouping live as the user types. Validation lives in
// `shared/utils.ts::isWarrenVoucher` so the same regex governs the
// `valueValid` gate (Submit button enable state) and the "did the
// user paste their pubkey by mistake" recovery hint rendered in
// `RedeemVoucherResponse`.
const WARREN_VOUCHER_RAW_LENGTH = 16;
// Crockford-32 alphabet uppercase: 0-9 + A-Z minus I/L/O/U. Passed
// as a JS regex char class to `FormattableTextInput`. MUST stay in
// sync with `warren_api::vouchers::VOUCHER_ALPHABET`.
const WARREN_VOUCHER_ALLOWED_CHARS = '[0-9A-HJKM-NP-TV-Z]';

interface IRedeemVoucherContextValue {
  onSubmit: () => void;
  value: string;
  setValue: (value: string) => void;
  valueValid: boolean;
  submittedValue: string;
  submitting: boolean;
  response?: VoucherResponse;
}

const contextProviderMissingError = new Error('<RedeemVoucherContext.Provider> is missing');

const RedeemVoucherContext = React.createContext<IRedeemVoucherContextValue>({
  onSubmit() {
    throw contextProviderMissingError;
  },
  get value(): string {
    throw contextProviderMissingError;
  },
  setValue(_) {
    throw contextProviderMissingError;
  },
  get valueValid(): boolean {
    throw contextProviderMissingError;
  },
  get submittedValue(): string {
    throw contextProviderMissingError;
  },
  get submitting(): boolean {
    throw contextProviderMissingError;
  },
  get response(): VoucherResponse {
    throw contextProviderMissingError;
  },
});

interface IRedeemVoucherProps {
  onSubmit?: () => void;
  onSuccess?: (newExpiry: string, secondsAdded: number) => void;
  onFailure?: () => void;
  children?: React.ReactNode;
}

export function RedeemVoucherContainer(props: IRedeemVoucherProps) {
  const { onSubmit, onSuccess, onFailure } = props;

  const { submitVoucher } = useAppContext();

  const [value, setValue] = useState('');
  const [submittedValue, setSubmittedValue] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [response, setResponse] = useState<VoucherResponse>();

  // Synchronous re-entry guard. The `submitting` state above is set via
  // `setSubmitting(true)` which React batches until the end of the
  // event handler; meanwhile the input's `disabled` prop has not yet
  // been committed to the DOM. If the user fires the submit twice in
  // quick succession (Enter key autorepeat, or click + Enter), the
  // closure-captured `submitting` value is still `false` for both
  // invocations and both calls race to `submitVoucher`. A `useRef`
  // updated synchronously closes that window without depending on
  // React's commit cycle. Reset to `false` in a `finally`-equivalent
  // path so a real failure doesn't permanently block re-submission.
  const inFlightRef = useRef(false);

  const valueValid = isWarrenVoucher(value);

  const onSubmitWrapper = useCallback(async () => {
    if (!valueValid || inFlightRef.current) {
      return;
    }
    inFlightRef.current = true;

    try {
      const submitTimestamp = Date.now();
      setSubmitting(true);
      setSubmittedValue(value);
      onSubmit?.();
      const response = await submitVoucher(value);

      // Show the spinner for at least half a second if it isn't successful.
      const submitDuration = Date.now() - submitTimestamp;
      if (response.type !== 'success' && submitDuration < 500) {
        await new Promise((resolve) => setTimeout(resolve, 500 - submitDuration));
      }

      setSubmitting(false);
      setResponse(response);
      if (response.type === 'success') {
        onSuccess?.(response.newExpiry, response.secondsAdded);
      } else {
        onFailure?.();
      }
    } finally {
      // Release the guard so a genuine retry (e.g. after a transient
      // network error) can re-submit. The button's `disabled` state
      // (driven by `submitting` / `response.type === 'success'`) keeps
      // the UX honest while in flight.
      inFlightRef.current = false;
    }
  }, [value, valueValid, onSubmit, submitVoucher, onSuccess, onFailure]);

  return (
    <RedeemVoucherContext.Provider
      value={{
        onSubmit: onSubmitWrapper,
        value,
        setValue,
        valueValid,
        submittedValue,
        submitting,
        response,
      }}>
      {props.children}
    </RedeemVoucherContext.Provider>
  );
}

interface IRedeemVoucherInputProps {
  className?: string;
}

export function RedeemVoucherInput(props: IRedeemVoucherInputProps) {
  const { value, setValue, onSubmit, submitting, response } = useContext(RedeemVoucherContext);
  const disabled = submitting || response?.type === 'success';

  const handleChange = useCallback(
    (value: string) => {
      setValue(value);
    },
    [setValue],
  );

  const onKeyPress = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === 'Enter') {
        onSubmit();
      }
    },
    [onSubmit],
  );

  return (
    <StyledInput
      className={props.className}
      // `FormattableTextInput` filters input character-by-character
      // against `allowedCharacters`. `WARREN_VOUCHER_ALLOWED_CHARS`
      // is the Crockford-32 char class - pasted I/L/O/U characters
      // are silently dropped (matching the backend's strict alphabet
      // gate, see `warren_api::vouchers::normalize_secret`'s "no
      // Crockford aliasing" rationale).
      allowedCharacters={WARREN_VOUCHER_ALLOWED_CHARS}
      separator="-"
      uppercaseOnly
      groupLength={4}
      maxLength={WARREN_VOUCHER_RAW_LENGTH}
      addTrailingSeparator
      disabled={disabled}
      value={value}
      placeholder={'XXXX-XXXX-XXXX-XXXX'}
      handleChange={handleChange}
      onKeyPress={onKeyPress}
    />
  );
}

export function RedeemVoucherResponse() {
  const { response, submitting, submittedValue } = useContext(RedeemVoucherContext);

  if (submitting) {
    return (
      <Flex alignItems="center" margin={{ top: 'small' }} gap="small">
        <Spinner size="medium" />
        <StyledProgressResponse>
          {messages.pgettext('redeem-voucher-view', 'Verifying voucher...')}
        </StyledProgressResponse>
      </Flex>
    );
  }

  if (response) {
    switch (response.type) {
      case 'success':
        return <StyledEmptyResponse />;
      case 'invalid':
        return (
          <>
            <StyledErrorResponse>
              {messages.pgettext('redeem-voucher-view', 'Voucher code is invalid.')}
            </StyledErrorResponse>
            {isWarrenPubKey(submittedValue) ? (
              <StyledWarrenPubKeyInfo>
                {messages.pgettext(
                  'redeem-voucher-view',
                  'It looks like you’ve entered a public key instead of a voucher code. If you would like to change the active account, please log out first.',
                )}
              </StyledWarrenPubKeyInfo>
            ) : null}
          </>
        );
      case 'already_used':
        return (
          <StyledErrorResponse>
            {messages.pgettext('redeem-voucher-view', 'Voucher code has already been used.')}
          </StyledErrorResponse>
        );
      case 'expired':
        return (
          <StyledErrorResponse>
            {messages.pgettext('redeem-voucher-view', 'Voucher code has expired.')}
          </StyledErrorResponse>
        );
      case 'not_ready':
      case 'error':
        return (
          <StyledErrorResponse>
            {messages.pgettext('redeem-voucher-view', 'An error occurred.')}
          </StyledErrorResponse>
        );
    }
  }

  return <StyledEmptyResponse />;
}

export function RedeemVoucherSubmitButton() {
  const { valueValid, onSubmit, submitting, response } = useContext(RedeemVoucherContext);
  const disabled = submitting || response?.type === 'success';

  return (
    <Button variant="success" disabled={!valueValid || disabled} onClick={onSubmit}>
      <Button.Text>
        {
          // TRANSLATORS: Button label for voucher redemption.
          messages.pgettext('redeem-voucher-view', 'Redeem')
        }
      </Button.Text>
    </Button>
  );
}

interface IRedeemVoucherAlertProps {
  show: boolean;
  onClose?: () => void;
}

export function RedeemVoucherAlert(props: IRedeemVoucherAlertProps) {
  const { submitting, response } = useContext(RedeemVoucherContext);
  const locale = useSelector((state) => state.userInterface.locale);

  if (response?.type === 'success') {
    const duration = formatRelativeDate(0, response.secondsAdded * 1000, {
      capitalize: true,
      displayMonths: true,
    });
    const expiry = formatDate(response.newExpiry, locale);

    return (
      <ModalAlert
        isOpen={props.show}
        buttons={[
          <Button key="gotit" onClick={props.onClose}>
            <Button.Text>{messages.gettext('Got it!')}</Button.Text>
          </Button>,
        ]}
        close={props.onClose}>
        <Flex justifyContent="center" margin={{ top: 'large', bottom: 'medium' }}>
          <IconBadge state="positive" />
        </Flex>
        <StyledTitle>
          {messages.pgettext('redeem-voucher-view', 'Voucher was successfully redeemed.')}
        </StyledTitle>
        <StyledLabel>
          {sprintf(messages.gettext('%(duration)s was added, account paid until %(expiry)s.'), {
            duration,
            expiry,
          })}
        </StyledLabel>
      </ModalAlert>
    );
  } else {
    return (
      <ModalAlert
        isOpen={props.show}
        buttons={[
          <RedeemVoucherSubmitButton key="submit" />,
          <Button key="cancel" disabled={submitting} onClick={props.onClose}>
            <Button.Text>
              {
                // TRANSLATORS: Cancel button label for voucher redemption.
                messages.pgettext('redeem-voucher-alert', 'Cancel')
              }
            </Button.Text>
          </Button>,
        ]}
        close={props.onClose}>
        <StyledLabel>
          {
            // TRANSLATORS: Input field label for voucher code.
            messages.pgettext('redeem-voucher-alert', 'Enter voucher code')
          }
        </StyledLabel>
        <RedeemVoucherInput />
        <RedeemVoucherResponse />
      </ModalAlert>
    );
  }
}

type RedeemVoucherButtonProps = ButtonProps;

export function RedeemVoucherButton(props: RedeemVoucherButtonProps) {
  const [showAlert, setShowAlert] = useState(false);

  const onClick = useCallback(() => setShowAlert(true), []);
  const onClose = useCallback(() => setShowAlert(false), []);

  return (
    <>
      <Button variant="success" onClick={onClick} {...props}>
        <Button.Text>
          {
            // TRANSLATORS: Button label for redeeming a voucher.
            messages.pgettext('redeem-voucher-alert', 'Redeem voucher')
          }
        </Button.Text>
      </Button>
      <RedeemVoucherContainer>
        <RedeemVoucherAlert show={showAlert} onClose={onClose} />
      </RedeemVoucherContainer>
    </>
  );
}
