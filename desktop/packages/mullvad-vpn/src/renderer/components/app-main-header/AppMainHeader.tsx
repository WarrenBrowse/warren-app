import { TunnelState } from '../../../shared/daemon-rpc-types';
import { Flex, HeaderProps, Logo, LogoProps, LogoState, MainHeader } from '../../lib/components';
import { useSelector } from '../../redux/store';
import { InitialFocus } from '../initial-focus';
import {
  AppMainHeaderBarAccountButton,
  AppMainHeaderPubKey,
  AppMainHeaderSettingsButton,
  AppMainHeaderTimeLeft,
} from './components';

export interface MainHeaderProps extends Omit<HeaderProps, 'variant' | 'size'> {
  variant?: HeaderProps['variant'] | 'basedOnConnectionStatus';
  size?: HeaderProps['size'] | 'basedOnLoginStatus';
  logoVariant?: LogoProps['variant'] | 'none';
  children?: React.ReactNode;
}

const AppMainHeader = ({
  logoVariant = 'both',
  variant: variantProp,
  size: sizeProp,
  children,
  ...props
}: MainHeaderProps) => {
  const connectionStatus = useSelector((state) => state.connection.status);

  const variant =
    variantProp === 'basedOnConnectionStatus'
      ? getVariantByTunnelState(connectionStatus)
      : variantProp;

  const loggedIn = useSelector((state) => state.account.status.type === 'ok');
  const size = sizeProp === 'basedOnLoginStatus' ? (loggedIn ? '2' : '1') : sizeProp;

  // Every coloured header state (success/error/pending) takes the dark logo for
  // strong contrast; only a neutral/default header keeps the light logo.
  const logoTone = variant && variant !== 'default' ? 'dark' : 'light';

  // Bula ducks into the burrow once we are actually connected, and pops his
  // masked face out otherwise.
  const logoState = getLogoStateByTunnelState(connectionStatus);

  return (
    <MainHeader variant={variant} size={size} {...props}>
      <Flex justifyContent="space-between" alignItems="center">
        <InitialFocus>
          {logoVariant !== 'none' ? (
            <Logo variant={logoVariant} tone={logoTone} state={logoState} />
          ) : (
            <div />
          )}
        </InitialFocus>
        <Flex gap="medium" alignItems="center">
          {children}
        </Flex>
      </Flex>
      {size == '2' && (
        <Flex justifyContent="space-between" alignItems="flex-end" margin={{ top: 'tiny' }}>
          <AppMainHeaderPubKey />
          <AppMainHeaderTimeLeft />
        </Flex>
      )}
    </MainHeader>
  );
};

const AppMainHeaderNamespace = Object.assign(AppMainHeader, {
  AccountButton: AppMainHeaderBarAccountButton,
  SettingsButton: AppMainHeaderSettingsButton,
});

export { AppMainHeaderNamespace as AppMainHeader };

const getVariantByTunnelState = (tunnelState: TunnelState): HeaderProps['variant'] => {
  switch (tunnelState.state) {
    case 'disconnected':
      return 'error';
    case 'connected':
      return 'success';
    // Calm in-between state while the tunnel is coming up or down.
    case 'connecting':
      return 'pending';
    case 'error':
      return !tunnelState.details.blockingError ? 'success' : 'error';
    case 'disconnecting':
      switch (tunnelState.details) {
        case 'block':
        case 'reconnect':
          return 'pending';
        case 'nothing':
          return 'error';
      }
  }
};

// Maps the tunnel state to which Bula mark the header shows.
// Only two states are active for now (hidden when connected, exposed otherwise).
// The 'blocked' branch is intentionally left as a TODO: once a dedicated
// kill-switch mark exists, return 'blocked' for the states where internet is
// blocked but the tunnel is not up (the 'error' blocking case and the
// 'disconnecting'/'block' case).
const getLogoStateByTunnelState = (tunnelState: TunnelState): LogoState => {
  switch (tunnelState.state) {
    case 'connected':
      return 'hidden';
    default:
      return 'exposed';
  }
};
