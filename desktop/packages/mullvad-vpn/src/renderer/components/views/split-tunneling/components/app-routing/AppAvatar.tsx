import styled from 'styled-components';

import { colors } from '../../../../../lib/foundations';
import { sourceSansPro } from '../../../../common-styles';

const StyledImage = styled.img<{ $size: number }>((props) => ({
  width: `${props.$size}px`,
  height: `${props.$size}px`,
  flexShrink: 0,
  objectFit: 'contain',
  userSelect: 'none',
}));

const StyledLetter = styled.span<{ $size: number }>((props) => ({
  ...sourceSansPro,
  width: `${props.$size}px`,
  height: `${props.$size}px`,
  flexShrink: 0,
  borderRadius: `${Math.round(props.$size * 0.24)}px`,
  backgroundColor: colors.blue60,
  color: colors.white,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  fontWeight: 700,
  fontSize: `${Math.round(props.$size * 0.45)}px`,
}));

export type AppAvatarProps = {
  name: string;
  icon?: string;
  size?: number;
};

// The app's own icon, or its initial on a tile for a program without one.
// Decorative: the name always sits next to it.
export function AppAvatar({ name, icon, size = 36 }: AppAvatarProps) {
  if (icon) {
    return <StyledImage src={icon} alt="" aria-hidden $size={size} draggable={false} />;
  }
  return (
    <StyledLetter aria-hidden $size={size}>
      {name.charAt(0).toLocaleUpperCase()}
    </StyledLetter>
  );
}
