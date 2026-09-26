import { Spacings, spacings } from '../../foundations';
import { LayoutSpacings } from './types';

export const all = (value: Spacings) => {
  const marginAll = spacings[value];
  return { margin: marginAll };
};

const vertical = (value: Spacings) => ({
  ...top(value),
  ...bottom(value),
});

const horizontal = (value: Spacings) => {
  return {
    ...left(value),
    ...right(value),
  };
};

const top = (value: Spacings) => {
  const marginTop = spacings[value];
  return {
    marginTop,
  };
};

// `left` and `right` follow the reading direction, so that they mirror in Arabic and Persian.
const right = (value: Spacings) => ({
  marginInlineEnd: spacings[value],
});

const bottom = (value: Spacings) => {
  const marginBottom = spacings[value];
  return {
    marginBottom,
  };
};

const left = (value: Spacings) => ({
  marginInlineStart: spacings[value],
});

export const margin: Record<keyof LayoutSpacings, (value: Spacings) => React.CSSProperties> = {
  all,
  vertical,
  horizontal,
  top,
  right,
  bottom,
  left,
};
