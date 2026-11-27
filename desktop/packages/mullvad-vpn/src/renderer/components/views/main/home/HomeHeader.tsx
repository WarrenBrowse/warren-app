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
  padding: 0 ${hero.logoInset}px;
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

  /* The shared icon button is a block that leaves its glyph at the top left:
     in the pill the glyph sits at the centre of its 38px square. Keyboard
     focus and hover tint that square rather than ringing it, which a frame
     around an icon in the glass pill made heavy. */
  & button {
    display: grid;
    place-items: center;
    border-radius: ${toolbar.radius - toolbar.paddingHorizontal}px;
  }
  && button:hover,
  && button:focus-visible {
    outline: none;
    background-color: color-mix(in srgb, currentColor 12%, transparent);
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
          <AppMainHeader.SupportButton />
          <AppMainHeader.ForumButton />
          <AppMainHeader.AccountButton />
          <AppMainHeader.SettingsButton />
        </StyledToolbar>
      </MainHeaderToneProvider>
    </StyledHeader>
  );
}
