import styled, { css } from 'styled-components';

import { surfaces } from '../../foundations';
import { Flex } from '../flex';
import { FeatureIndicatorText } from './components';
import { FeatureIndicatorProvider } from './FeatureIndicatorContext';

export type FeatureIndicatorProps = {
  variant?: 'primary' | 'transparent' | 'error';
} & React.ComponentPropsWithRef<'button'>;

// Opaque chips in the theme's paper, like the card under them: a translucent
// chip took the hue of whatever landscape it floated over.
const styles = {
  variants: {
    primary: {
      backgroundColor: surfaces.card,
      borderColor: surfaces.line,
      borderColorHover: surfaces.textMuted,
      borderColorPressed: surfaces.text,
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
      backgroundColor: surfaces.exposedWell,
      borderColor: surfaces.exposed,
      borderColorHover: surfaces.textMuted,
      borderColorPressed: surfaces.text,
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

      border-radius: 7px;
      background: ${variant.backgroundColor};
      border: 0.5px solid ${variant.borderColor};
      box-shadow: ${variantProp === 'transparent' ? 'none' : `0 1.5px 5px ${surfaces.shadowSoft}`};

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
        outline: 2px solid ${surfaces.text};
        outline-offset: -2px;
      }
    `;
  }}
`;

const StyledFlex = styled(Flex)`
  padding: 5.5px 8px;
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
