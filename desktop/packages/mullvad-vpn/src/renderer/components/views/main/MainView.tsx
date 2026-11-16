import styled from 'styled-components';

import { AppCountriesIndicator } from '../../../features/app-routing/components';
import { PortForwardingIndicator } from '../../../features/port-forwarding/components';
import { homeTokens, surfaces } from '../../../lib/foundations';
import { AppMainHeader } from '../../app-main-header';
import { BetaBadge } from '../../beta-badge';
import CountryBackdrop from '../../CountryBackdrop';
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
`;

// The hero takes whatever height the panel leaves, and the scenery frames
// itself in it; the panel below keeps its natural height.
const StyledHero = styled.div`
  position: relative;
  flex: 1;
  min-height: 0;
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
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
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
        <StyledBeta>
          <BetaBadge variant="home" />
        </StyledBeta>
      </StyledPanel>
      <AppMainHeader.Footer />
    </StyledHome>
  );
}
