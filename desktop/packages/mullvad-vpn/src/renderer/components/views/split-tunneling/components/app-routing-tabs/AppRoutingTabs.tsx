import React from 'react';
import styled from 'styled-components';

import { includeOnlyTabState } from '../../../../../../shared/app-routing';
import { messages } from '../../../../../../shared/gettext';
import { AppRoutingTab } from '../../../../../../shared/ipc-types';
import { colors, spacings } from '../../../../../lib/foundations';
import { useSelector } from '../../../../../redux/store';
import { smallText, tinyText } from '../../../../common-styles';
import { useSplitTunnelingContext } from '../../SplitTunnelingContext';

const TABS: AppRoutingTab[] = ['bypass', 'countries', 'include-only'];

export function tabLabel(tab: AppRoutingTab): string {
  switch (tab) {
    case 'bypass':
      return messages.pgettext('split-tunneling-view', 'Bypass VPN');
    case 'countries':
      return messages.pgettext('split-tunneling-view', 'Country per app');
    case 'include-only':
      return messages.pgettext('split-tunneling-view', 'VPN only for');
  }
}

export const tabId = (tab: AppRoutingTab) => `app-routing-tab-${tab}`;
export const tabPanelId = (tab: AppRoutingTab) => `app-routing-panel-${tab}`;
const COMING_SOON_ID = 'app-routing-include-only-coming-soon';

// Same look as the location view's Entry/Exit scope bar.
const StyledTabList = styled.div({
  display: 'flex',
  margin: `0 ${spacings.medium} ${spacings.medium}`,
  backgroundColor: colors.blue40,
  borderRadius: '13px',
  overflow: 'hidden',
});

const StyledTab = styled.button<{ $selected: boolean }>(smallText, (props) => ({
  flex: 1,
  flexBasis: 0,
  padding: '6px 6px',
  border: 'none',
  color: colors.white,
  textAlign: 'center',
  cursor: 'default',
  backgroundColor: props.$selected ? colors.green : colors.transparent,
  '&&:not(:disabled):hover': {
    backgroundColor: props.$selected ? colors.green : colors.blue60,
  },
  '&&:disabled': {
    color: colors.whiteAlpha40,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '-2px',
    borderRadius: '13px',
  },
}));

const StyledComingSoon = styled.p(tinyText, {
  margin: `calc(-1 * ${spacings.small}) ${spacings.medium} ${spacings.medium}`,
  color: colors.whiteAlpha60,
  fontWeight: 400,
});

export function AppRoutingTabs() {
  const { tab, setTab } = useSplitTunnelingContext();
  const splitMode = useSelector((state) => state.settings.appRouting.splitMode);
  const includeOnlyComingSoon =
    includeOnlyTabState(window.env.platform, splitMode) === 'coming-soon';
  const tabs = includeOnlyComingSoon
    ? TABS.filter((candidate) => candidate !== 'include-only')
    : TABS;
  const refs = React.useRef<Array<HTMLButtonElement | null>>([]);
  const registerButton = React.useCallback((index: number, element: HTMLButtonElement | null) => {
    refs.current[index] = element;
  }, []);

  const onKeyDown = React.useCallback(
    (event: React.KeyboardEvent) => {
      // A tab that is not offered is skipped.
      const index = tabs.indexOf(tab);
      let next: number | undefined;
      switch (event.key) {
        case 'ArrowRight':
          next = (index + 1) % tabs.length;
          break;
        case 'ArrowLeft':
          next = (index - 1 + tabs.length) % tabs.length;
          break;
        case 'Home':
          next = 0;
          break;
        case 'End':
          next = tabs.length - 1;
          break;
      }
      if (next !== undefined) {
        event.preventDefault();
        setTab(tabs[next]);
        refs.current[TABS.indexOf(tabs[next])]?.focus();
      }
    },
    [setTab, tab, tabs],
  );

  return (
    <>
      <StyledTabList
        role="tablist"
        aria-label={messages.pgettext('split-tunneling-view', 'App routing')}
        onKeyDown={onKeyDown}>
        {TABS.map((candidate, index) => (
          <Tab
            key={candidate}
            tab={candidate}
            index={index}
            selected={candidate === tab}
            disabled={!tabs.includes(candidate)}
            onSelect={setTab}
            registerButton={registerButton}
          />
        ))}
      </StyledTabList>
      {includeOnlyComingSoon && (
        <StyledComingSoon id={COMING_SOON_ID} data-testid="include-only-coming-soon">
          {
            // TRANSLATORS: One line under the App routing tabs on Windows, where
            // TRANSLATORS: the "VPN only for" mode is not available yet.
            messages.pgettext('split-tunneling-view', 'VPN only for is coming soon on Windows.')
          }
        </StyledComingSoon>
      )}
    </>
  );
}

type TabProps = {
  tab: AppRoutingTab;
  index: number;
  selected: boolean;
  disabled: boolean;
  onSelect: (tab: AppRoutingTab) => void;
  registerButton: (index: number, element: HTMLButtonElement | null) => void;
};

function Tab({ tab, index, selected, disabled, onSelect, registerButton }: TabProps) {
  const setRef = React.useCallback(
    (element: HTMLButtonElement | null) => registerButton(index, element),
    [index, registerButton],
  );
  const select = React.useCallback(() => onSelect(tab), [onSelect, tab]);

  return (
    <StyledTab
      ref={setRef}
      id={tabId(tab)}
      role="tab"
      type="button"
      aria-selected={selected}
      aria-controls={tabPanelId(tab)}
      // Roving focus: Tab reaches the selected tab, the arrows move between them.
      tabIndex={selected ? 0 : -1}
      disabled={disabled}
      aria-disabled={disabled}
      aria-describedby={disabled ? COMING_SOON_ID : undefined}
      $selected={selected}
      onClick={select}>
      {tabLabel(tab)}
    </StyledTab>
  );
}
