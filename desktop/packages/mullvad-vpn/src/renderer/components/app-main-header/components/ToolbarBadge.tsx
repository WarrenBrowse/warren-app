import styled from 'styled-components';

import { homeTokens, surfaces } from '../../../lib/foundations';

const { toolbar } = homeTokens;

// The mockup's unread dot: inset from the corner of the 38px button, ringed in
// the toolbar's glass so it detaches from the glyph under it. The parent is the
// glyph's own box, centred in the button.
export const ToolbarBadge = styled.span`
  position: absolute;
  top: ${toolbar.badgeInset - (toolbar.button - toolbar.icon) / 2}px;
  right: ${toolbar.badgeInset - (toolbar.button - toolbar.icon) / 2}px;
  width: ${toolbar.badge}px;
  height: ${toolbar.badge}px;
  border-radius: 50%;
  background-color: ${surfaces.homeBadge};
  box-shadow: 0 0 0 ${toolbar.badgeRing}px ${surfaces.homeBadgeRing};
  pointer-events: none;
`;

export const ToolbarGlyph = styled.span`
  position: relative;
  display: grid;
  place-items: center;
`;
