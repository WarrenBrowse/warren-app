import React from 'react';
import styled from 'styled-components';

import { Text } from '../../lib/components';
import { colors, Radius, spacings } from '../../lib/foundations';

// Renders a 12 (or 24) word BIP39 mnemonic as a 3-column numbered grid.
// Shared between the onboarding wallet step (`OnboardingWalletView` -
// generate mode) and the settings backup view (`KeysView` - reveal
// mode). The component is purely presentational; revealing the secret
// behind a blur overlay is the caller's responsibility via the
// optional `revealed` prop.

const StyledGrid = styled.div<{ $revealed: boolean }>`
  display: grid;
  /* Each cell must hold the longest BIP39 word (8 chars) plus its
     index without clipping, so the column count derives from the
     container width instead of a fixed 3 columns: the narrow menubar
     window gets 2 columns, wider containers get 3+. A fixed 1fr
     3-column layout made long words spill past the card edge. */
  grid-template-columns: repeat(auto-fill, minmax(88px, 1fr));
  gap: ${spacings.small};
  padding: ${spacings.small};
  border: 1px solid ${colors.whiteAlpha20};
  border-radius: ${Radius.radius4};
  background-color: ${colors.darkerBlue50};
  font-family: 'Source Code Pro', Menlo, Consolas, monospace;
  filter: ${({ $revealed }) => ($revealed ? 'none' : 'blur(8px)')};
  cursor: ${({ $revealed }) => ($revealed ? 'default' : 'pointer')};
  transition: filter 0.15s ease;
`;

const StyledWord = styled.div`
  display: flex;
  align-items: baseline;
  gap: ${spacings.tiny};
  min-width: 0;
  padding: ${spacings.tiny} 0;
`;

const StyledIndex = styled.span`
  min-width: 20px;
  text-align: right;
  color: ${colors.whiteAlpha60};
  font-size: 11px;
`;

export type MnemonicGridProps = {
  mnemonic: string;
  // When false, the grid is rendered behind a blur overlay. Defaults
  // to true (shown).
  revealed?: boolean;
  // Click handler. Useful when `revealed` is false and the caller
  // wants click-to-reveal behavior.
  onClick?: () => void;
  'data-testid'?: string;
};

export function MnemonicGrid({ mnemonic, revealed = true, onClick, ...rest }: MnemonicGridProps) {
  const words = React.useMemo(() => mnemonic.split(/\s+/).filter((w) => w.length > 0), [mnemonic]);

  return (
    <StyledGrid
      role="textbox"
      aria-readonly="true"
      $revealed={revealed}
      onClick={onClick}
      data-testid={rest['data-testid']}>
      {words.map((word, idx) => (
        <StyledWord key={idx}>
          <StyledIndex>{idx + 1}.</StyledIndex>
          <Text variant="bodySmall" color="white" as="span">
            {word}
          </Text>
        </StyledWord>
      ))}
    </StyledGrid>
  );
}
