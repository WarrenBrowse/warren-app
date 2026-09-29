import styled, { css } from 'styled-components';

import { Button } from '../../../../../../../lib/components';
import { StyledButtonText } from '../../../../../../../lib/components/button/components';
import { FontFamilies, surfaces } from '../../../../../../../lib/foundations';

// neutral: the location and shuffle buttons, a raised fill of the card itself.
// connect, disconnect, cancel: the action, white on a fill that says what the
// click does rather than the state the tunnel is in.
export type CardButtonTone = 'neutral' | 'connect' | 'disconnect' | 'cancel';

const tones: Record<
  CardButtonTone,
  { background: string; hover: string; pressed: string; border: string; text: string }
> = {
  neutral: {
    background: surfaces.button,
    hover: surfaces.buttonHover,
    pressed: surfaces.buttonPressed,
    border: surfaces.buttonLine,
    text: surfaces.text,
  },
  connect: {
    background: surfaces.connect,
    hover: surfaces.connectHover,
    pressed: surfaces.connectPressed,
    border: 'transparent',
    text: surfaces.actionText,
  },
  disconnect: {
    background: surfaces.disconnect,
    hover: surfaces.disconnectHover,
    pressed: surfaces.disconnectPressed,
    border: 'transparent',
    text: surfaces.actionText,
  },
  cancel: {
    background: surfaces.cancel,
    hover: surfaces.cancelHover,
    pressed: surfaces.cancelPressed,
    border: 'transparent',
    text: surfaces.actionText,
  },
};

// The shared Button reads its fills from custom properties, so a tone only has
// to redefine them; the text colour is the one thing set on a child.
export const CardButton = styled(Button)<{ $tone: CardButtonTone }>(({ $tone }) => {
  const tone = tones[$tone];
  return css`
    --background: ${tone.background};
    --hover: ${tone.hover};
    --pressed: ${tone.pressed};
    --disabled: ${tone.background};
    --radius: 6px;

    min-height: 32px;
    border: 0.5px solid ${tone.border};

    & ${StyledButtonText} {
      --color: ${tone.text};
      font-family: ${FontFamilies.openSans};
      font-size: 13px;
      line-height: 18px;
      font-weight: 600;
    }

    &:disabled ${StyledButtonText} {
      opacity: 0.5;
    }

    &:focus-visible {
      outline-color: ${surfaces.text};
    }
  `;
});
