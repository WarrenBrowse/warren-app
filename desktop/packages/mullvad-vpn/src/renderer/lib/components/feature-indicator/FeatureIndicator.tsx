import styled, { css } from 'styled-components';

import { homeTokens, surfaces } from '../../foundations';
import { Flex } from '../flex';
import { FeatureIndicatorText } from './components';
import { FeatureIndicatorProvider } from './FeatureIndicatorContext';

export type FeatureIndicatorProps = {
  variant?: 'primary' | 'transparent' | 'error';
} & React.ComponentPropsWithRef<'button'>;

// Outlined chips on the home page under the controls.
const styles = {
  variants: {
    primary: {
      backgroundColor: 'transparent',
      borderColor: surfaces.homeChipLine,
      borderColorHover: surfaces.homeTextSecondary,
      borderColorPressed: surfaces.homeText,
    },
    transparent: {
      backgroundColor: 'transparent',
      borderColor: 'transparent',
      borderColorHover: 'transparent',
      borderColorPressed: 'transparent',
    },
    // Alerting chip: a port-forward (or other feature) in a blocked/error
    // state surfaces in red so the user notices it on the main screen
    // without opening settings.
    error: {
      backgroundColor: 'transparent',
      borderColor: surfaces.homeDisconnectLine,
      borderColorHover: surfaces.homeDisconnectText,
      borderColorPressed: surfaces.homeText,
    },
  },
};

const StyledFeatureIndicator = styled.button<{
  $variant: FeatureIndicatorProps['variant'];
  $clickable?: boolean;
}>`
  ${({ $variant: variantProp = 'primary', $clickable }) => {
    const variant = styles.variants[variantProp];
    return css`
      display: flex;
      align-items: center;
      box-sizing: border-box;
      height: ${homeTokens.chips.height}px;

      border-radius: 99px;
      background: ${variant.backgroundColor};
      border: ${homeTokens.chips.borderWidth}px solid ${variant.borderColor};

      ${() => {
        if ($clickable) {
          return css`
            &&:not(:disabled):hover {
              border-color: ${variant.borderColorHover};
            }
            &&:not(:disabled):active {
              border-color: ${variant.borderColorPressed};
            }
          `;
        }
        return null;
      }}

      &&:disabled {
        background: var(--disabled);
      }
      &&:focus-visible {
        outline: 2px solid ${surfaces.homeText};
        outline-offset: -2px;
      }
    `;
  }}
`;

const StyledFlex = styled(Flex)`
  padding: ${homeTokens.chips.paddingVertical}px ${homeTokens.chips.paddingHorizontal}px;
`;

function FeatureIndicator({
  ref,
  variant,
  children,
  disabled,
  style,
  onClick,
  ...props
}: FeatureIndicatorProps) {
  const clickable = !disabled && !!onClick;
  return (
    <FeatureIndicatorProvider disabled={disabled}>
      <StyledFeatureIndicator
        ref={ref}
        $variant={variant}
        $clickable={clickable}
        disabled={disabled}
        onClick={onClick}
        {...props}>
        <StyledFlex alignItems="center">{children}</StyledFlex>
      </StyledFeatureIndicator>
    </FeatureIndicatorProvider>
  );
}

const FeatureIndicatorNamespace = Object.assign(FeatureIndicator, {
  Text: FeatureIndicatorText,
});

export { FeatureIndicatorNamespace as FeatureIndicator };
