import React from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../../shared/gettext';
import { AppRoutingTab } from '../../../../../../shared/ipc-types';
import { colors, spacings } from '../../../../../lib/foundations';
import { smallText } from '../../../../common-styles';
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
  '&&:hover': {
    backgroundColor: props.$selected ? colors.green : colors.blue60,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '-2px',
    borderRadius: '13px',
  },
}));

export function AppRoutingTabs() {
  const { tab, setTab } = useSplitTunnelingContext();
  const refs = React.useRef<Array<HTMLButtonElement | null>>([]);
  const registerButton = React.useCallback((index: number, element: HTMLButtonElement | null) => {
    refs.current[index] = element;
  }, []);

  const onKeyDown = React.useCallback(
    (event: React.KeyboardEvent) => {
      const index = TABS.indexOf(tab);
      let next: number | undefined;
      switch (event.key) {
        case 'ArrowRight':
          next = (index + 1) % TABS.length;
          break;
        case 'ArrowLeft':
          next = (index - 1 + TABS.length) % TABS.length;
          break;
        case 'Home':
          next = 0;
          break;
        case 'End':
          next = TABS.length - 1;
          break;
      }
      if (next !== undefined) {
        event.preventDefault();
        setTab(TABS[next]);
        refs.current[next]?.focus();
      }
    },
    [setTab, tab],
  );

  return (
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
          onSelect={setTab}
          registerButton={registerButton}
        />
      ))}
    </StyledTabList>
  );
}

type TabProps = {
  tab: AppRoutingTab;
  index: number;
  selected: boolean;
  onSelect: (tab: AppRoutingTab) => void;
  registerButton: (index: number, element: HTMLButtonElement | null) => void;
};

function Tab({ tab, index, selected, onSelect, registerButton }: TabProps) {
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
      $selected={selected}
      onClick={select}>
      {tabLabel(tab)}
    </StyledTab>
  );
}
