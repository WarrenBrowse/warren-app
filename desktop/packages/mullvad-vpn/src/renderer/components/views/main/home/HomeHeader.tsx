import styled from 'styled-components';

import { Logo, MainHeaderToneProvider } from '../../../../lib/components';
import { useResolvedTheme } from '../../../../lib/components/theme/use-document-theme';
import { homeTokens, surfaces } from '../../../../lib/foundations';
import { AppMainHeader } from '../../../app-main-header';
import { InitialFocus } from '../../../initial-focus';

const { hero, toolbar } = homeTokens;

const StyledHeader = styled.header`
  position: absolute;
  top: ${hero.headerInset}px;
  left: ${hero.headerInset}px;
  right: ${hero.headerInset}px;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: ${toolbar.height}px;
`;

// The logo is inked like the art, so it keeps its dark ink in both themes and
// sits on a paper halo that separates it from whatever the sky holds.
const StyledLogo = styled.div`
  display: flex;
  align-items: center;
  height: ${toolbar.height}px;
  padding: 0 4px;
  filter: drop-shadow(0 0 1px rgba(250, 247, 240, 0.95))
    drop-shadow(0 0 4px rgba(250, 247, 240, 0.85)) drop-shadow(0 0 14px rgba(250, 247, 240, 0.7));
`;

const StyledToolbar = styled.div`
  --main-header-button-size: ${toolbar.button}px;
  --main-header-icon-size: ${toolbar.icon}px;
  display: flex;
  align-items: center;
  height: ${toolbar.height}px;
  padding: 0 ${toolbar.paddingHorizontal}px;
  border-radius: ${toolbar.radius}px;
  background-color: ${surfaces.homeToolbar};
  color: ${surfaces.homeText};
  backdrop-filter: blur(${toolbar.blur}px);
  box-shadow:
    inset 0 0 0 1px ${surfaces.homeToolbarLine},
    0 4px 14px -6px ${surfaces.homeToolbarShadow};

  /* The shared icon button rings a circle sized for its own default glyph;
     in the pill the hit area is a rounded square, and so is its ring. */
  & button {
    border-radius: ${toolbar.radius - toolbar.paddingHorizontal}px;
  }
  && button:focus-visible {
    outline: 2px solid currentColor;
    outline-offset: -2px;
  }
`;

export function HomeHeader() {
  // The toolbar is a dark glass in the dark theme and a paper one in the light
  // theme, so its glyphs take the opposite tone.
  const iconTone = useResolvedTheme() === 'dark' ? 'light' : 'dark';
  return (
    <StyledHeader>
      <InitialFocus>
        <StyledLogo>
          <Logo variant="both" height={hero.logoHeight} wordmarkTone="dark" />
        </StyledLogo>
      </InitialFocus>
      <MainHeaderToneProvider value={iconTone}>
        <StyledToolbar>
          <AppMainHeader.ForumButton />
          <AppMainHeader.AccountButton />
          <AppMainHeader.SettingsButton />
        </StyledToolbar>
      </MainHeaderToneProvider>
    </StyledHeader>
  );
}
