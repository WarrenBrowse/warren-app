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

  const logoState = getLogoStateByTunnelState(connectionStatus);

  return (
    <MainHeader variant={variant} size={size} {...props}>
      <Flex justifyContent="space-between" alignItems="center">
        <InitialFocus>
          {logoVariant !== 'none' ? (
            <Logo variant={logoVariant} state={logoState} />
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

const getLogoStateByTunnelState = (tunnelState: TunnelState): LogoState => {
  switch (tunnelState.state) {
    case 'connected':
      return 'hidden';
    default:
      return 'exposed';
  }
};
