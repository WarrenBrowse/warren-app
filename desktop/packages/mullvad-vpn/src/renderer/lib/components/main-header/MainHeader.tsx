import styled from 'styled-components';

import { Colors, colors } from '../../foundations';
import { TransientProps } from '../../types';
import { Flex } from '../flex';
import { IconButtonTone } from '../icon-button';
import { MainHeaderIconButton } from './components';
import { MainHeaderToneProvider } from './MainHeaderContext';

// 'pending' is the calm in-between state shown while (dis)connecting, between
// the connected (success/green) and disconnected (error/red) states.
// 'transparent' lets the header float over the full-bleed scenery instead of
// painting a coloured bar.
type HeaderVariant = 'default' | 'success' | 'error' | 'pending' | 'transparent';

export type HeaderProps = React.PropsWithChildren<{
  size?: '1' | '2';
  variant?: HeaderVariant;
  // Icon/text tone: 'dark' when floating over bright scenery, 'light' over the
  // coloured/charcoal bar.
  tone?: IconButtonTone;
}>;

const sizes = {
  '1': '68px',
  '2': '80px',
};

const variants: Record<HeaderVariant, Colors> = {
  default: 'blue',
  error: 'red',
  success: 'green',
  pending: 'yellow',
  transparent: 'transparent',
};

const StyledHeader = styled.header<TransientProps<HeaderProps>>(
  ({ $size = '1', $variant = 'default' }) => {
    const backgroundColor = colors[variants[$variant]];
    return {
      height: sizes[$size],
      minHeight: sizes[$size],

      backgroundColor,
      transition: 'height 250ms ease-in-out, min-height 250ms ease-in-out',
    };
  },
);

const MainHeader = ({
  size = '1',
  variant = 'default',
  tone = 'light',
  children,
  ...props
}: HeaderProps) => {
  return (
    <StyledHeader $size={size} $variant={variant} {...props}>
      <MainHeaderToneProvider value={tone}>
        <Flex
          flexDirection="column"
          justifyContent="center"
          margin={{
            horizontal: 'medium',
            top: 'medium',
            bottom: 'small',
          }}>
          {children}
        </Flex>
      </MainHeaderToneProvider>
    </StyledHeader>
  );
};

const MainHeaderNamespace = Object.assign(MainHeader, {
  IconButton: MainHeaderIconButton,
});

export { MainHeaderNamespace as MainHeader };
