import React from 'react';
import { createGlobalStyle } from 'styled-components';

import {
  colorPrimitives,
  colors,
  fontFamilies,
  fontSizes,
  fontWeights,
  lineHeights,
  radius,
  spacingPrimitives,
} from '../../foundations/variables';

type VariablesProps = React.PropsWithChildren<object>;

// macOS rounds frameless windows at the OS level; Windows does not, so there we
// clip #app to a rounded rect ourselves (body stays transparent so the corners
// show through). Linux keeps square corners: its window is WM-decorated.
const roundWindowCorners = window.env.platform === 'win32';

const GlobalStyle = createGlobalStyle`
  :root {
    ${Object.entries({
      ...spacingPrimitives,
      ...colorPrimitives,
      ...radius,
      ...fontFamilies,
      ...fontSizes,
      ...fontWeights,
      ...lineHeights,
    }).reduce((styleString, [key, value]) => ({ ...styleString, [key]: value }), {})}
  }

  body {
    background-color: ${roundWindowCorners ? 'transparent' : colors.darkBlue};
  }

  ${
    roundWindowCorners
      ? `#app {
        border-radius: 12px;
        overflow: hidden;
        background-color: ${colors.darkBlue};
      }`
      : ''
  }
`;

export const Theme = ({ children }: VariablesProps) => {
  return (
    <>
      <GlobalStyle />
      {children}
    </>
  );
};
