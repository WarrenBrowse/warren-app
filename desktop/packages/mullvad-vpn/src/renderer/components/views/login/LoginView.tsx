import { useCallback, useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { Button, Checkbox, Flex, Spinner, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { Link } from '../../../lib/components/link';
import { View } from '../../../lib/components/view';
import { colors, Radius, spacings } from '../../../lib/foundations';
import { formatHtml } from '../../../lib/html-formatter';
import { IconBadge } from '../../../lib/icon-badge';
import { useSelector } from '../../../redux/store';
import { AppMainHeader } from '../../app-main-header';
import {
  CopyMnemonicButton,
  countMnemonicWords,
  MnemonicGrid,
  MnemonicTextarea,
  normalizeMnemonic,
} from '../../warren-mnemonic';
import {
  StyledBlockMessage,
  StyledBlockMessageContainer,
  StyledBlockTitle,
  StyledStatusIcon,
} from './LoginStyles';

// The logged-out entry screen, redesigned around the Warren wallet
// model: an identity IS a BIP39 recovery phrase, not a public key you
// type in. Two paths:
// - "Create a new account": the daemon mints a fresh identity (and logs
//   in); the GUI then holds on this screen (redux `backup-pending`)
//   showing the 12-word recovery phrase and requires the user to
//   confirm they saved it before proceeding (mandatory backup step).
// - "I already have an account": the user pastes their recovery phrase,
//   which the daemon validates + hot-swaps to restore the identity.

const DangerCallout = styled.div`
  padding: ${spacings.tiny} ${spacings.small};
  border-radius: ${Radius.radius4};
  background-color: ${colors.redAlpha40};
  border: 1px solid ${colors.red40};
`;

type Mode = 'pick' | 'restore';

export function LoginView() {
  const { createNewAccount, finishAccountBackup, getWarrenMnemonic, setWarrenMnemonic } =
    useAppContext();

  const status = useSelector((state) => state.account.status);
  const tunnelState = useSelector((state) => state.connection.status);
  const isPerformingPostUpgrade = useSelector(
    (state) => state.userInterface.isPerformingPostUpgrade,
  );

  const showBlockMessage =
    tunnelState.state === 'error' ||
    (tunnelState.state === 'disconnected' && tunnelState.lockedDown);

  const isBackup = status.type === 'backup-pending';
  const backupPubkey = status.type === 'backup-pending' ? status.pubkey : null;
  // The daemon is busy minting + logging into the new identity.
  const creating = status.type === 'logging in';
  const createFailed = status.type === 'failed' ? status.error.message : null;

  const [mode, setMode] = useState<Mode>('pick');
  const [mnemonic, setMnemonic] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [restoreInput, setRestoreInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Once the daemon has minted the identity (backup-pending), fetch the
  // freshly generated phrase to display for backup.
  useEffect(() => {
    if (isBackup && mnemonic === null) {
      getWarrenMnemonic()
        .then((phrase) => {
          if (phrase) {
            setMnemonic(phrase);
          } else {
            setError(
              messages.pgettext('login-view', 'Could not read the new recovery phrase. Try again.'),
            );
          }
        })
        .catch((e: unknown) => {
          log.error(`getWarrenMnemonic failed: ${(e as Error).message}`);
          setError(
            messages.pgettext('login-view', 'Could not read the new recovery phrase. Try again.'),
          );
        });
    }
  }, [isBackup, mnemonic, getWarrenMnemonic]);

  // Drop the secret from React memory when leaving the view.
  useEffect(() => {
    return () => {
      setMnemonic(null);
      setRestoreInput('');
    };
  }, []);

  const goRestore = useCallback(() => {
    setError(null);
    setMode('restore');
  }, []);

  const goPick = useCallback(() => {
    setError(null);
    setMode('pick');
  }, []);

  const onCreate = useCallback(() => {
    setError(null);
    setMnemonic(null);
    setConfirmed(false);
    void createNewAccount();
  }, [createNewAccount]);

  const onConfirmBackup = useCallback(() => {
    if (backupPubkey) {
      finishAccountBackup(backupPubkey);
    }
  }, [finishAccountBackup, backupPubkey]);

  const restoreWordCount = countMnemonicWords(restoreInput);
  const restoreValid = restoreWordCount === 12 || restoreWordCount === 24;

  const onRestore = useCallback(async () => {
    setError(null);
    setBusy(true);
    try {
      await setWarrenMnemonic(normalizeMnemonic(restoreInput));
      // On success the daemon logs in and emits a device event that
      // navigates away from this screen.
    } catch (e) {
      log.error(`Failed to restore identity: ${(e as Error).message}`);
      setError(
        messages.pgettext(
          'login-view',
          'Invalid recovery phrase. Check the words and their order, then try again.',
        ),
      );
    } finally {
      setBusy(false);
    }
  }, [restoreInput, setWarrenMnemonic]);

  const title = formTitle(isBackup, mode, isPerformingPostUpgrade);
  const statusIcon = getStatusIcon(isPerformingPostUpgrade, creating, createFailed !== null);

  return (
    <View>
      <AppMainHeader>
        <AppMainHeader.SettingsButton disabled={isPerformingPostUpgrade || creating} />
      </AppMainHeader>
      <View.Content>
        <View.Container flexDirection="column" horizontalMargin="medium" justifyContent="center">
          <FlexColumn gap="medium">
            {(showBlockMessage || statusIcon) && (
              <Flex justifyContent="center">{showBlockMessage ? <BlockMessage /> : statusIcon}</Flex>
            )}

            <View.Container
              gap="medium"
              horizontalMargin="small"
              justifyContent="center"
              flexDirection="column">
              <FlexColumn gap="small">
                <Text as="h1" variant={isBackup ? 'titleMedium' : 'titleBig'} aria-live="polite">
                  {title}
                </Text>

                {(error || createFailed) && (
                  <Text variant="bodySmall" color="red">
                    {error ?? createFailed}
                  </Text>
                )}

                {isBackup ? (
                  <BackupStep
                    mnemonic={mnemonic}
                    confirmed={confirmed}
                    onConfirmedChange={setConfirmed}
                    onContinue={onConfirmBackup}
                  />
                ) : mode === 'restore' ? (
                  <RestoreStep
                    value={restoreInput}
                    onValueChange={setRestoreInput}
                    canSubmit={restoreValid && !busy}
                    busy={busy}
                    onSubmit={onRestore}
                    onBack={goPick}
                  />
                ) : (
                  <PickStep creating={creating} onCreate={onCreate} onRestore={goRestore} />
                )}
              </FlexColumn>
            </View.Container>
          </FlexColumn>
        </View.Container>
      </View.Content>
    </View>
  );
}

function formTitle(isBackup: boolean, mode: Mode, isPerformingPostUpgrade?: boolean): string {
  if (isPerformingPostUpgrade) {
    return messages.pgettext('login-view', 'Upgrading...');
  }
  if (isBackup) {
    // TRANSLATORS: Title of the step that shows the new recovery phrase.
    return messages.pgettext('login-view', 'Back up your recovery phrase');
  }
  if (mode === 'restore') {
    // TRANSLATORS: Title of the step where the user enters their recovery phrase.
    return messages.pgettext('login-view', 'Restore your account');
  }
  // TRANSLATORS: Title of the logged-out welcome screen.
  return messages.pgettext('login-view', 'Welcome to Warren');
}

function getStatusIcon(
  isPerformingPostUpgrade: boolean | undefined,
  creating: boolean,
  failed: boolean,
) {
  if (isPerformingPostUpgrade || creating) {
    return (
      <StyledStatusIcon>
        <Spinner size="big" />
      </StyledStatusIcon>
    );
  }
  if (failed) {
    return (
      <StyledStatusIcon>
        <IconBadge state="negative" />
      </StyledStatusIcon>
    );
  }
  return null;
}

interface PickStepProps {
  creating: boolean;
  onCreate: () => void;
  onRestore: () => void;
}

function PickStep({ creating, onCreate, onRestore }: PickStepProps) {
  return (
    <FlexColumn gap="medium">
      <Text variant="bodySmall" color="whiteAlpha60">
        {messages.pgettext(
          'login-view',
          'Warren uses a non-custodial wallet: your account is a 12-word recovery phrase that only you hold.',
        )}
      </Text>
      <Button variant="success" onClick={onCreate} disabled={creating} data-testid="login-create">
        {creating ? (
          <Spinner />
        ) : (
          // TRANSLATORS: Button that creates a brand-new account.
          <Button.Text>{messages.pgettext('login-view', 'Create a new account')}</Button.Text>
        )}
      </Button>
      <Button variant="primary" onClick={onRestore} disabled={creating} data-testid="login-restore">
        <Button.Text>
          {
            // TRANSLATORS: Button that starts restoring an existing account from a recovery phrase.
            messages.pgettext('login-view', 'I already have an account')
          }
        </Button.Text>
      </Button>
    </FlexColumn>
  );
}

interface BackupStepProps {
  mnemonic: string | null;
  confirmed: boolean;
  onConfirmedChange: (value: boolean) => void;
  onContinue: () => void;
}

function BackupStep({ mnemonic, confirmed, onConfirmedChange, onContinue }: BackupStepProps) {
  if (!mnemonic) {
    return (
      <Flex justifyContent="center">
        <Spinner size="big" />
      </Flex>
    );
  }
  return (
    <FlexColumn gap="small">
      <DangerCallout>
        <Text variant="footnoteMini" color="white">
          {messages.pgettext(
            'login-view',
            'Write these 12 words down and keep them safe. They are the only way to restore your account if you lose this device.',
          )}
        </Text>
      </DangerCallout>

      <MnemonicGrid mnemonic={mnemonic} revealed data-testid="login-mnemonic-grid" />
      <CopyMnemonicButton mnemonic={mnemonic} data-testid="login-mnemonic-copy" />

      <Checkbox checked={confirmed} onCheckedChange={onConfirmedChange}>
        <Flex gap="small" alignItems="flex-start">
          <Checkbox.Trigger>
            <Checkbox.Input />
          </Checkbox.Trigger>
          <Checkbox.Label>
            {messages.pgettext(
              'login-view',
              'I have written down my recovery phrase in a safe place.',
            )}
          </Checkbox.Label>
        </Flex>
      </Checkbox>

      <Button
        variant="success"
        onClick={onContinue}
        disabled={!confirmed}
        data-testid="login-backup-continue">
        <Button.Text>{messages.gettext('Continue')}</Button.Text>
      </Button>
    </FlexColumn>
  );
}

interface RestoreStepProps {
  value: string;
  onValueChange: (value: string) => void;
  canSubmit: boolean;
  busy: boolean;
  onSubmit: () => void;
  onBack: () => void;
}

function RestoreStep({
  value,
  onValueChange,
  canSubmit,
  busy,
  onSubmit,
  onBack,
}: RestoreStepProps) {
  return (
    <FlexColumn gap="medium">
      <Text variant="bodySmall" color="whiteAlpha60">
        {messages.pgettext(
          'login-view',
          'Enter your 12-word recovery phrase, separated by spaces, to restore your account on this device.',
        )}
      </Text>
      <MnemonicTextarea
        value={value}
        onValueChange={onValueChange}
        placeholder="abandon abandon abandon ... about"
        data-testid="login-restore-input"
      />
      <Button
        variant="success"
        onClick={onSubmit}
        disabled={!canSubmit}
        data-testid="login-restore-submit">
        {busy ? (
          <Spinner />
        ) : (
          // TRANSLATORS: Button that restores the account from the entered recovery phrase.
          <Button.Text>{messages.pgettext('login-view', 'Restore account')}</Button.Text>
        )}
      </Button>
      <Link as="button" onClick={onBack} disabled={busy}>
        <Link.Text>{messages.gettext('Back')}</Link.Text>
      </Link>
    </FlexColumn>
  );
}

function BlockMessage() {
  const { setLockdownMode, disconnectTunnel } = useAppContext();
  const tunnelState = useSelector((state) => state.connection.status);
  const lockdownMode = tunnelState.state === 'disconnected' && tunnelState.lockedDown;

  const unlock = useCallback(() => {
    if (lockdownMode) {
      void setLockdownMode(false);
    }

    if (tunnelState.state === 'error') {
      void disconnectTunnel('gui-login-unblock');
    }
  }, [lockdownMode, tunnelState, setLockdownMode, disconnectTunnel]);

  const lockdownModeSettingName = messages.pgettext('vpn-settings-view', 'Lockdown mode');
  const message = formatHtml(
    lockdownMode
      ? sprintf(
          // TRANSLATORS: This is a warning message shown when the app is blocking the users
          // TRANSLATORS: internet connection while logged out.
          // TRANSLATORS: Available placeholder:
          // TRANSLATORS: %(lockdownModeSettingName)s - The translation of "Lockdown mode"
          messages.pgettext(
            'login-view',
            '<b>%(lockdownModeSettingName)s</b> is enabled. Disable it to unblock your connection.',
          ),
          { lockdownModeSettingName },
        )
      : // TRANSLATORS: This is a warning message shown when the app is blocking the users
        // TRANSLATORS: internet connection while logged out.
        messages.pgettext('login-view', 'Our kill switch is currently blocking your connection.'),
  );
  const buttonText = lockdownMode ? messages.gettext('Disable') : messages.gettext('Unblock');

  return (
    <StyledBlockMessageContainer>
      <StyledBlockTitle>{messages.gettext('Blocking internet')}</StyledBlockTitle>
      <StyledBlockMessage>{message}</StyledBlockMessage>
      <Button variant="destructive" onClick={unlock}>
        <Button.Text>{buttonText}</Button.Text>
      </Button>
    </StyledBlockMessageContainer>
  );
}
