import styled from 'styled-components';

import { AppCountriesIndicator } from '../../../features/app-routing/components';
import { PortForwardingIndicator } from '../../../features/port-forwarding/components';
import { homeTokens, surfaces } from '../../../lib/foundations';
import { useBoolean } from '../../../lib/utility-hooks';
import { AppMainHeader } from '../../app-main-header';
import CountryBackdrop from '../../CountryBackdrop';
import {
  COMPACT_TRANSITION_MS,
  HERO_COMPACT_RATIO,
  HERO_HEIGHT_RATIO,
} from '../../CountryBackdrop/framing';
import NotificationArea from '../../NotificationArea';
import { FeatureIndicators, MultihopIndicator } from './components';
import { HomeControls } from './home/HomeControls';
import { HomeHeader } from './home/HomeHeader';
import { HomeStatus } from './home/HomeStatus';

const { panel, chips, toolbar, hero, notice } = homeTokens;

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

// The hero keeps the mockup's proportions (300 high on a 380 width) and
// shrinks while the connection details are open; the panel below takes the
// rest of the window.
const StyledHero = styled.div<{ $compact: boolean }>`
  position: relative;
  flex-shrink: 0;
  height: calc(100vw * ${(props) => (props.$compact ? HERO_COMPACT_RATIO : HERO_HEIGHT_RATIO)});
  transition: height ${COMPACT_TRANSITION_MS}ms cubic-bezier(0.3, 0.7, 0.2, 1);
  @media (prefers-reduced-motion: reduce) {
    transition: none;
  }
`;

// Transient notices float under the toolbar, over the art, so that they never
// move the controls.
const StyledNotices = styled.div`
  position: absolute;
  top: ${hero.headerInset + toolbar.height + notice.marginTop}px;
  left: ${hero.headerInset}px;
  right: ${hero.headerInset}px;
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

export function MainView() {
  const [detailsOpen, , closeDetails, toggleDetails] = useBoolean();
  return (
    <StyledHome>
      <StyledHero $compact={detailsOpen}>
        <CountryBackdrop compact={detailsOpen} />
        <HomeHeader />
        <StyledNotices>
          <NotificationArea />
        </StyledNotices>
      </StyledHero>
      <StyledPanel>
        <HomeStatus
          detailsOpen={detailsOpen}
          toggleDetails={toggleDetails}
          closeDetails={closeDetails}
        />
        <HomeControls />
        <StyledChips>
          <FeatureIndicators />
          <MultihopIndicator />
          <PortForwardingIndicator />
          <AppCountriesIndicator />
        </StyledChips>
        <StyledSpacer />
      </StyledPanel>
      <AppMainHeader.Footer />
    </StyledHome>
  );
}
