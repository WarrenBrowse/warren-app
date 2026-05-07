import React, { useCallback } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { Url } from '../../../../shared/constants';
import { AccountDataError, WarrenPubKey } from '../../../../shared/daemon-rpc-types';
import { messages } from '../../../../shared/gettext';
import { isWarrenPubKey } from '../../../../shared/utils';
import { useAppContext } from '../../../context';
import useActions from '../../../lib/actionsHook';
import { Box, Button, Flex, Icon, Spinner, Text, TitleMedium } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { Label } from '../../../lib/components/label';
import { Link } from '../../../lib/components/link';
import { View } from '../../../lib/components/view';
import { colors } from '../../../lib/foundations';
import { formatHtml } from '../../../lib/html-formatter';
import { IconBadge } from '../../../lib/icon-badge';
import { formatWarrenPubKey } from '../../../lib/pubkey';
import accountActions from '../../../redux/account/actions';
import { LoginState } from '../../../redux/account/reducers';
import { useSelector } from '../../../redux/store';
import Accordion from '../../Accordion';
import { AppMainHeader } from '../../app-main-header';
import ClearAccountHistoryDialog from './ClearAccountHistoryDialog';
import CreateAccountDialog from './CreateAccountDialog';
import {
  StyledAccountDropdownContainer,
  StyledAccountDropdownItem,
  StyledAccountDropdownItemButton,
  StyledAccountDropdownItemIconButton,
  StyledAccountInputBackdrop,
  StyledAccountInputGroup,
  StyledBlockMessage,
  StyledBlockMessageContainer,
  StyledBlockTitle,
  StyledDropdownSpacer,
  StyledInput,
  StyledLine,
  StyledStatusIcon,
} from './LoginStyles';

export function LoginView() {
  const { openUrl, login, clearAccountHistory, createNewAccount } = useAppContext();
  const { resetLoginError, updatePubKey } = useActions(accountActions);

  const { pubkey, pubkeyHistory, status } = useSelector((state) => state.account);

  const tunnelState = useSelector((state) => state.connection.status);
  const showBlockMessage =
    tunnelState.state === 'error' ||
    (tunnelState.state === 'disconnected' && tunnelState.lockedDown);

  const isPerformingPostUpgrade = useSelector(
    (state) => state.userInterface.isPerformingPostUpgrade,
  );

  return (
    <Login
      pubkey={pubkey}
      pubkeyHistory={pubkeyHistory}
      loginState={status}
      showBlockMessage={showBlockMessage}
      openExternalLink={openUrl}
      login={login}
      resetLoginError={resetLoginError}
      updatePubKey={updatePubKey}
      clearAccountHistory={clearAccountHistory}
      createNewAccount={createNewAccount}
      isPerformingPostUpgrade={isPerformingPostUpgrade}
    />
  );
}

interface IProps {
  pubkey?: WarrenPubKey;
  pubkeyHistory?: WarrenPubKey;
  loginState: LoginState;
  showBlockMessage: boolean;
  openExternalLink: (type: Url) => void;
  login: (pubkey: WarrenPubKey) => void;
  resetLoginError: () => void;
  updatePubKey: (pubkey: WarrenPubKey) => void;
  clearAccountHistory: () => Promise<void>;
  createNewAccount: () => void;
  isPerformingPostUpgrade?: boolean;
}

interface IState {
  isActive: boolean;
  clearAccountHistoryDialogVisible: boolean;
  createAccountDialogVisible: boolean;
}

const WARREN_PUBKEY_HEX_LEN = 64;

class Login extends React.Component<IProps, IState> {
  public state: IState = {
    isActive: true,
    clearAccountHistoryDialogVisible: false,
    createAccountDialogVisible: false,
  };

  private accountInput = React.createRef<HTMLInputElement>();
  private shouldResetLoginError = false;

  constructor(props: IProps) {
    super(props);

    if (props.loginState.type === 'failed') {
      this.shouldResetLoginError = true;
    }
  }

  public componentDidUpdate(prevProps: IProps, _prevState: IState) {
    if (
      this.props.loginState.type !== prevProps.loginState.type &&
      this.props.loginState.type === 'failed'
    ) {
      this.shouldResetLoginError = true;

      // focus on login field when failed to log in
      this.accountInput.current?.focus();
    }
  }

  public render() {
    const allowInteraction = this.allowInteraction();
    return (
      <View>
        <AppMainHeader>
          <AppMainHeader.SettingsButton disabled={!allowInteraction} />
        </AppMainHeader>
        <View.Content>
          <View.Container flexDirection="column" horizontalMargin="medium" justifyContent="center">
            <FlexColumn gap="medium">
              <Flex justifyContent="center">
                {this.props.showBlockMessage ? <BlockMessage /> : this.getStatusIcon()}
              </Flex>

              <View.Container
                gap="large"
                horizontalMargin="small"
                justifyContent="center"
                flexDirection="column">
                <FlexColumn gap="small">
                  <Text as="h1" variant="titleBig" aria-live="polite">
                    {this.formTitle()}
                  </Text>

                  {this.createLoginForm()}
                </FlexColumn>
                <Flex justifyContent="center">
                  <StyledLine margin={{ vertical: 'small', right: 'small' }} />
                  <Text variant="labelTinySemiBold">
                    {
                      // TRANSLATORS: Text shown between two horizontal lines above the "create account" button.
                      // TRANSLATORS: In this context it is used to separate the users alternative of logging in
                      // TRANSLATORS: or creating a new account, "Login or Create a new account".
                      messages.pgettext('login-view', 'Or')
                    }
                  </Text>
                  <StyledLine margin={{ vertical: 'small', left: 'small' }} />
                </Flex>
              </View.Container>
              {this.createFooter()}
            </FlexColumn>
          </View.Container>
        </View.Content>
      </View>
    );
  }

  private onFocus = () => {
    this.setState({ isActive: true });
  };

  private onBlur = () => {
    this.setState({ isActive: false });
  };

  private onSubmit = (event?: React.FormEvent) => {
    event?.preventDefault();

    if (this.pubkeyValid()) {
      this.props.login(this.props.pubkey!);
    }
  };

  private onInputChange = (pubkey: string) => {
    // reset error when user types in the new pubkey
    if (this.shouldResetLoginError) {
      this.shouldResetLoginError = false;
      this.props.resetLoginError();
    }

    this.props.updatePubKey(pubkey);
  };

  private formTitle() {
    if (this.props.isPerformingPostUpgrade) {
      return messages.pgettext('login-view', 'Upgrading...');
    }

    switch (this.props.loginState.type) {
      case 'logging in':
      case 'too many devices':
        return this.props.loginState.method === 'existing_account'
          ? messages.pgettext('login-view', 'Logging in...')
          : messages.pgettext('login-view', 'Creating account...');
      case 'failed':
        return this.props.loginState.method === 'existing_account'
          ? messages.pgettext('login-view', 'Login failed')
          : messages.pgettext('login-view', 'Error');
      case 'ok':
        return this.props.loginState.method === 'existing_account'
          ? messages.pgettext('login-view', 'Logged in')
          : messages.pgettext('login-view', 'Account created');
      default:
        return messages.pgettext('login-view', 'Login');
    }
  }

  private formSubtitle() {
    if (this.props.isPerformingPostUpgrade) {
      return messages.pgettext('login-view', 'Finishing upgrade.');
    }

    switch (this.props.loginState.type) {
      case 'failed':
        return this.props.loginState.method === 'existing_account'
          ? this.errorString(this.props.loginState.error)
          : messages.pgettext('login-view', 'Failed to create account');
      case 'too many devices':
        return messages.pgettext('login-view', 'Too many devices');
      case 'logging in':
        return this.props.loginState.method === 'existing_account'
          ? messages.pgettext('login-view', 'Checking public key')
          : messages.pgettext('login-view', 'Please wait');
      case 'ok':
        return this.props.loginState.method === 'existing_account'
          ? messages.pgettext('login-view', 'Valid public key')
          : messages.pgettext('login-view', 'Logged in');
      default:
        return messages.pgettext('login-view', 'Enter your public key');
    }
  }

  private errorString(error: AccountDataError['error']): string {
    switch (error) {
      case 'invalid-account':
        // TRANSLATORS: Error message shown above login input when trying to login with a
        // TRANSLATORS: non-existent public key.
        return messages.pgettext('login-view', 'Invalid public key');
      case 'too-many-devices':
        // TRANSLATORS: Error message shown above login input when trying to login to an account
        // TRANSLATORS: with too many registered devices.
        return messages.pgettext('login-view', 'Too many devices');
      case 'list-devices':
        // TRANSLATORS: Error message shown trying to login but the app fails
        // TRANSLATORS: to fetch the list of registered devices.
        return messages.gettext('Failed to fetch list of devices');
      case 'communication':
        return 'api.mullvad.net is blocked, please check your firewall';
      default:
        return messages.pgettext('login-view', 'Unknown error');
    }
  }

  private getStatusIcon() {
    return <StyledStatusIcon>{this.getStatusIconPath()}</StyledStatusIcon>;
  }

  private getStatusIconPath() {
    if (this.props.isPerformingPostUpgrade) {
      return <Spinner size="big" />;
    }

    switch (this.props.loginState.type) {
      case 'logging in':
        return <Spinner size="big" />;
      case 'failed':
        return <IconBadge state="negative" />;
      case 'ok':
        return <IconBadge state="positive" />;
      default:
        return null;
    }
  }

  private allowInteraction() {
    return (
      !this.props.isPerformingPostUpgrade &&
      this.props.loginState.type !== 'logging in' &&
      this.props.loginState.type !== 'ok' &&
      this.props.loginState.type !== 'too many devices'
    );
  }

  private allowCreateAccount() {
    const { pubkey } = this.props;
    return this.allowInteraction() && (pubkey === undefined || pubkey.length === 0);
  }

  private pubkeyValid(): boolean {
    const { pubkey } = this.props;
    return pubkey !== undefined && isWarrenPubKey(pubkey);
  }

  private shouldShowAccountHistory() {
    return this.allowInteraction() && this.props.pubkeyHistory !== undefined;
  }

  private onSelectAccountFromHistory = (pubkey: string) => {
    this.props.updatePubKey(pubkey);
    this.props.login(pubkey);
  };

  private onClearAccountHistory = () => {
    this.setState({ clearAccountHistoryDialogVisible: true });
  };

  private onConfirmClearAccountHistory = () => {
    this.hideClearAccountHistoryDialog();
    void this.clearAccountHistory();
  };

  private hideClearAccountHistoryDialog = () => {
    this.setState({ clearAccountHistoryDialogVisible: false });
  };

  private async clearAccountHistory() {
    try {
      await this.props.clearAccountHistory();

      // TODO: Remove account from memory
    } catch {
      // TODO: Show error
    }
  }

  private onCreateNewAccount = () => {
    if (this.props.pubkeyHistory !== undefined) {
      this.setState({ createAccountDialogVisible: true });
    } else {
      this.onConfirmCreateNewAccount();
    }
  };

  private onConfirmCreateNewAccount = () => {
    this.props.createNewAccount();
    this.hideCreateAccountDialog();
  };

  private hideCreateAccountDialog = () => {
    this.setState({ createAccountDialogVisible: false });
  };

  private createLoginForm() {
    const inputId = 'warren-pubkey-input';
    const allowInteraction = this.allowInteraction();
    const allowLogin = allowInteraction && this.pubkeyValid();
    const hasError =
      this.props.loginState.type === 'failed' &&
      this.props.loginState.method === 'existing_account';

    return (
      <>
        <Flex flexDirection="column" gap="tiny">
          <Label
            htmlFor={inputId}
            variant="labelTinySemiBold"
            color="whiteAlpha60"
            data-testid="subtitle">
            {this.formSubtitle()}
          </Label>
          <form onSubmit={this.onSubmit}>
            <FlexColumn gap="large">
              <StyledAccountInputGroup
                $active={allowInteraction && this.state.isActive}
                $editable={allowInteraction}
                $error={hasError}>
                <StyledAccountInputBackdrop>
                  <StyledInput
                    id={inputId}
                    allowedCharacters="[0-9a-fA-F]"
                    separator=" "
                    groupLength={8}
                    maxLength={WARREN_PUBKEY_HEX_LEN}
                    placeholder="00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000"
                    value={this.props.pubkey || ''}
                    disabled={!allowInteraction}
                    onFocus={this.onFocus}
                    onBlur={this.onBlur}
                    handleChange={this.onInputChange}
                    autoFocus={true}
                    ref={this.accountInput}
                    aria-autocomplete="list"
                  />
                </StyledAccountInputBackdrop>
                <Accordion expanded={this.shouldShowAccountHistory()}>
                  <StyledAccountDropdownContainer>
                    <AccountDropdown
                      item={this.props.pubkeyHistory}
                      onSelect={this.onSelectAccountFromHistory}
                      onRemove={this.onClearAccountHistory}
                    />
                  </StyledAccountDropdownContainer>
                </Accordion>
              </StyledAccountInputGroup>
              <Button
                type="submit"
                variant="success"
                disabled={!allowLogin}
                aria-label={
                  // TRANSLATORS: This is used by screenreaders to communicate the login button.
                  messages.pgettext('accessibility', 'Login')
                }>
                <Button.Text>
                  {
                    // TRANSLATORS: Label for the login button.
                    messages.pgettext('login-view', 'Login')
                  }
                </Button.Text>
              </Button>
            </FlexColumn>
          </form>
        </Flex>

        <ClearAccountHistoryDialog
          visible={this.state.clearAccountHistoryDialogVisible}
          onConfirm={this.onConfirmClearAccountHistory}
          onHide={this.hideClearAccountHistoryDialog}
        />
      </>
    );
  }

  private createFooter() {
    return (
      <>
        <Flex flexDirection="column" gap="small" alignItems="center">
          <Link as="button" onClick={this.onCreateNewAccount} disabled={!this.allowCreateAccount()}>
            <Link.Text>
              {
                // TRANSLATORS: Text in button that allows user to create a new account.
                messages.pgettext('login-view', 'Create a new account')
              }
            </Link.Text>
          </Link>
        </Flex>
        <CreateAccountDialog
          visible={this.state.createAccountDialogVisible}
          onConfirm={this.onConfirmCreateNewAccount}
          onHide={this.hideCreateAccountDialog}
        />
      </>
    );
  }
}

interface IAccountDropdownProps {
  item?: WarrenPubKey;
  onSelect: (value: WarrenPubKey) => void;
  onRemove: (value: WarrenPubKey) => void;
}

function AccountDropdown(props: IAccountDropdownProps) {
  const pubkey = props.item;
  if (!pubkey) {
    return null;
  }
  const label = formatWarrenPubKey(pubkey);
  return (
    <AccountDropdownItem
      value={pubkey}
      label={label}
      onSelect={props.onSelect}
      onRemove={props.onRemove}
    />
  );
}

interface AccountDropdownItemProps {
  label: string;
  value: WarrenPubKey;
  onRemove: (value: WarrenPubKey) => void;
  onSelect: (value: WarrenPubKey) => void;
}

const StyledIcon = styled(Icon)({
  backgroundColor: colors.whiteOnBlue20,
});

function AccountDropdownItem({ label, onRemove, onSelect, value }: AccountDropdownItemProps) {
  const handleSelect = useCallback(() => {
    onSelect(value);
  }, [onSelect, value]);

  const handleRemove = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      // Prevent login form from submitting
      event.preventDefault();
      onRemove(value);
    },
    [onRemove, value],
  );

  const itemId = React.useId();

  return (
    <>
      <StyledDropdownSpacer />
      <StyledAccountDropdownItem>
        <Flex alignItems="center" justifyContent="space-between" flexGrow={1}>
          <StyledAccountDropdownItemButton
            id={itemId}
            onClick={handleSelect}
            type="button"
            aria-label={sprintf(
              // TRANSLATORS: This is used by screenreaders to communicate logging in with a saved public key.
              // TRANSLATORS: Available placeholders:
              // TRANSLATORS: %(pubkey)s - the saved public key
              messages.pgettext('accessibility', 'Login with public key %(pubkey)s'),
              {
                pubkey: label,
              },
            )}>
            <TitleMedium color="blue80">{label}</TitleMedium>
          </StyledAccountDropdownItemButton>
          <Box $height="48px" $width="48px" center>
            <StyledAccountDropdownItemIconButton
              onClick={handleRemove}
              type="button"
              aria-controls={itemId}
              aria-label={sprintf(
                // TRANSLATORS: This is used by screenreaders to communicate the "x" button next to a saved public key.
                // TRANSLATORS: Available placeholders:
                // TRANSLATORS: %(pubkey)s - the public key to the left of the button
                messages.pgettext('accessibility', 'Forget public key %(pubkey)s'),
                {
                  pubkey: label,
                },
              )}>
              <StyledIcon icon="cross-circle" size="small" />
            </StyledAccountDropdownItemIconButton>
          </Box>
        </Flex>
      </StyledAccountDropdownItem>
    </>
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
      : // This makes the translator comment appear on it's own line.
        // TRANSLATORS: This is a warning message shown when the app is blocking the users
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
