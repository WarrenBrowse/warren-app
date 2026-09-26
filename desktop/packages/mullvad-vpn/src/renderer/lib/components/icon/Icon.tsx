import styled, { css } from 'styled-components';

import { Colors, colors } from '../../foundations';
import { icons } from './types';

export type IconProps = {
  icon: keyof typeof icons;
  size?: 'tiny' | 'small' | 'medium' | 'large' | 'big';
  color?: Colors;
  className?: string;
} & React.HTMLAttributes<HTMLDivElement>;

// Icons that point along the reading direction (forward, back) and so face the other way in
// right-to-left languages. The individual `scale` property composes with any `transform` a
// caller sets.
const MIRRORED_ICONS: ReadonlySet<string> = new Set([
  'chevron-left',
  'chevron-left-circle',
  'chevron-right',
  'chevron-right-circle',
]);

export const StyledIcon = styled.div<{
  $color: string;
  $size: number;
  $src: string;
  $mirrored?: boolean;
}>`
  ${({ $size, $src, $color, $mirrored }) => {
    return css`
      flex-shrink: 0;
      width: ${$size}px;
      height: ${$size}px;
      mask: url(${$src}) no-repeat center;
      mask-size: contain;
      background-color: ${$color};
      ${$mirrored &&
      css`
        &:dir(rtl) {
          scale: -1 1;
        }
      `}
    `;
  }}
`;

export const iconSizes = {
  tiny: 14,
  small: 18,
  medium: 24,
  large: 32,
  big: 48,
};

export const Icon = ({
  icon: iconProp,
  size = 'medium',
  color: colorProp = 'white',
  ...props
}: IconProps) => {
  const icon = icons[iconProp];
  const src = iconProp.startsWith('data:') ? iconProp : `assets/icons/${icon}.svg`;

  const color = colors[colorProp];
  return (
    <StyledIcon
      $src={src}
      $size={iconSizes[size]}
      $color={color}
      $mirrored={MIRRORED_ICONS.has(iconProp)}
      role="img"
      {...props}
    />
  );
};
