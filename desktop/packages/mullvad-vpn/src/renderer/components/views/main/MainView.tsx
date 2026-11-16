import styled from 'styled-components';

import { AppCountriesIndicator } from '../../../features/app-routing/components';
import { PortForwardingIndicator } from '../../../features/port-forwarding/components';
import { homeTokens, surfaces } from '../../../lib/foundations';
import { AppMainHeader } from '../../app-main-header';
import { BetaBadge } from '../../beta-badge';
import CountryBackdrop from '../../CountryBackdrop';
import { HERO_HEIGHT_RATIO } from '../../CountryBackdrop/framing';
import NotificationArea from '../../NotificationArea';
import { FeatureIndicators, MultihopIndicator } from './components';
import { HomeControls } from './home/HomeControls';
import { HomeHeader } from './home/HomeHeader';
import { HomeStatus } from './home/HomeStatus';

const { panel, chips, toolbar, hero } = homeTokens;

const StyledHome = styled.div`
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background-color: ${surfaces.homePage};
  /* The app thins its glyphs globally; the mockup is drawn with the system
     smoothing, which gives Instrument Sans its intended weight. */
  -webkit-font-smoothing: auto;
`;

// The hero keeps the mockup's proportions (300 high on a 360 width); the panel
// below takes the rest of the window.
const StyledHero = styled.div`
  position: relative;
  flex-shrink: 0;
  aspect-ratio: ${1 / HERO_HEIGHT_RATIO};
`;

// Transient notices float under the toolbar, over the art, so that they never
// move the controls.
const StyledNotices = styled.div`
  position: absolute;
  top: ${hero.headerInset + toolbar.height + 8}px;
  left: 0;
  right: 0;
  z-index: 2;
`;

const StyledPanel = styled.main`
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
`;

// The room the mockup leaves between the chips and the footer.
const StyledSpacer = styled.div`
  flex: 1;
`;

// One row of chips is reserved even with none, so the hero does not reframe
// every time the tunnel comes up.
const StyledChips = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: ${chips.gap}px;
  min-height: ${chips.height}px;
  margin-top: ${chips.marginTop}px;
  padding: 0 ${panel.paddingHorizontal}px;
`;

const StyledBeta = styled.div`
  padding: 10px 0;
  &:empty {
    display: none;
  }
`;

export function MainView() {
  return (
    <StyledHome>
      <StyledHero>
        <CountryBackdrop />
        <HomeHeader />
        <StyledNotices>
          <NotificationArea />
        </StyledNotices>
      </StyledHero>
      <StyledPanel>
        <HomeStatus />
        <HomeControls />
        <StyledChips>
          <FeatureIndicators />
          <MultihopIndicator />
          <PortForwardingIndicator />
          <AppCountriesIndicator />
        </StyledChips>
        <StyledSpacer />
        <StyledBeta>
          <BetaBadge variant="home" />
        </StyledBeta>
      </StyledPanel>
      <AppMainHeader.Footer />
    </StyledHome>
  );
}
