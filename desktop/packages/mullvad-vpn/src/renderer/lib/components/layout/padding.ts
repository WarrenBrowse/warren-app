import { Spacings, spacings } from '../../foundations';
import { LayoutSpacings } from './types';

export const all = (value: Spacings) => {
  const paddingAll = spacings[value];
  return { padding: paddingAll };
};

const vertical = (value: Spacings) => ({
  ...top(value),
  ...bottom(value),
});

const horizontal = (value: Spacings) => ({
  ...left(value),
  ...right(value),
});

const top = (value: Spacings) => {
  const paddingTop = spacings[value];
  return {
    paddingTop,
  };
};

// `left` and `right` follow the reading direction, so that they mirror in Arabic and Persian.
const right = (value: Spacings) => ({
  paddingInlineEnd: spacings[value],
});

const bottom = (value: Spacings) => {
  const paddingBottom = spacings[value];
  return {
    paddingBottom,
  };
};

const left = (value: Spacings) => ({
  paddingInlineStart: spacings[value],
});

export const padding: Record<keyof LayoutSpacings, (value: Spacings) => React.CSSProperties> = {
  all,
  vertical,
  horizontal,
  top,
  right,
  bottom,
  left,
};
