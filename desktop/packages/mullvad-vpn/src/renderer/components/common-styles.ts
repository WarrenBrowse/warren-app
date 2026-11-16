import React from 'react';

import { colors, spacings } from '../lib/foundations';

export const uiFont: React.CSSProperties = {
  fontFamily: 'var(--font-family-sans)',
};

export const tinyText = {
  ...uiFont,
  fontSize: '12px',
  fontWeight: 600,
  lineHeight: '18px',
};

export const smallText = {
  ...uiFont,
  fontSize: '14px',
  fontWeight: 600,
  lineHeight: '20px',
  color: colors.whiteAlpha80,
};

export const smallNormalText = {
  ...smallText,
  fontWeight: 'normal',
};

export const normalText = {
  ...uiFont,
  fontSize: '15px',
  lineHeight: '18px',
};

export const largeText = {
  ...uiFont,
  fontWeight: 600,
  fontSize: '18px',
  lineHeight: '24px',
};

export const buttonText = {
  ...largeText,
  color: colors.white,
};

export const bigText = {
  ...uiFont,
  fontSize: '24px',
  fontWeight: 700,
  lineHeight: '28px',
};

export const hugeText = {
  ...uiFont,
  fontSize: '32px',
  fontWeight: 700,
  lineHeight: '34px',
  color: colors.white,
};

export const measurements = {
  rowMinHeight: '48px',
  horizontalViewMargin: spacings.medium,
  verticalViewMargin: spacings.large,
  rowVerticalMargin: spacings.large,
  buttonVerticalMargin: spacings.medium,
};
