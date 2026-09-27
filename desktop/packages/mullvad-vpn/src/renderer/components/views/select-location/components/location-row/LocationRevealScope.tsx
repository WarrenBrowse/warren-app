import styled from 'styled-components';

import { StyledListItemRoot } from '../../../../../lib/components/list-item';
import { StyledListItemTrigger } from '../../../../../lib/components/list-item/components';
import { colors } from '../../../../../lib/foundations';

/**
 * Keeps each row's secondary buttons out of sight until the row is pointed at
 * or holds the focus, so the list reads as places, not as controls.
 */
export const LocationRevealScope = styled.div`
  display: contents;

  [data-reveal] {
    opacity: 0;
    transition: opacity 150ms ease-out;
  }

  ${StyledListItemRoot}:hover [data-reveal],
  ${StyledListItemRoot}:focus-within [data-reveal],
  [data-reveal][data-open='true'] {
    opacity: 1;
  }

  /* The lists button shares the row's surface, so it lights up with the row. */
  ${StyledListItemTrigger}:hover ~ * [data-lists-action] {
    background-color: ${colors.whiteOnBlue10};
  }
`;
