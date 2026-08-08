import styled from 'styled-components';

import { TunnelState } from '../../../shared/daemon-rpc-types';
import { Flex, HeaderProps, Logo, LogoProps, LogoState, MainHeader } from '../../lib/components';
import { useSelector } from '../../redux/store';
import { InitialFocus } from '../initial-focus';
import {
  AppMainFooter,
  AppMainHeaderBarAccountButton,
  AppMainHeaderForumButton,
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

// Ease the lockup off the corner: a small down-right shift gives the ears room
// to breathe against the window edges. A transform (not margin) so the header
// layout and the buttons' baseline stay untouched.
const StyledLogoSlot = styled.span`
  display: inline-flex;
  transform: translate(5px, 3px);
`;

// The wordmark's letters sit in the bottom half of its box (the ears tower
// above), so the header row bottom-aligns and the buttons ride the same nudge
// as the logo: their centre then lands on the letter band, the optical
// alignment the eye expects, instead of hovering up at ear height.
// Mirrors the logo slot's 5px inset from the opposite edge so both ends of the
// header keep the same breathing room.
const StyledHeaderButtons = styled(Flex)`
  transform: translate(-5px, 6px);
`;

const AppMainHeader = ({
  logoVariant = 'both',
  variant: variantProp,
  size: sizeProp,
  tone = 'light',
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
    <MainHeader variant={variant} size={size} tone={tone} {...props}>
      <Flex justifyContent="space-between" alignItems="flex-end">
        <InitialFocus>
          {logoVariant !== 'none' ? (
            <StyledLogoSlot>
              <Logo variant={logoVariant} state={logoState} wordmarkTone={tone} />
            </StyledLogoSlot>
          ) : (
            <div />
          )}
        </InitialFocus>
        <StyledHeaderButtons gap="large" alignItems="center">
          {children}
        </StyledHeaderButtons>
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
  ForumButton: AppMainHeaderForumButton,
  SettingsButton: AppMainHeaderSettingsButton,
  Footer: AppMainFooter,
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
