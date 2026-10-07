import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import {
  type AppRoute,
  appRules,
  defaultRoute,
  resolveApplications,
  ruleLine,
} from '../../../../../../shared/app-routing';
import { messages } from '../../../../../../shared/gettext';
import { RouteStatusLine } from '../../../../../features/app-routing/components';
import { useAppRouting, useExitChoiceNames } from '../../../../../features/app-routing/hooks';
import { colors } from '../../../../../lib/foundations';
import { useSelector } from '../../../../../redux/store';
import { sourceSansPro } from '../../../../common-styles';
import { type RoutingTarget, useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { AppAvatar } from './AppAvatar';
import { WarningGlyph } from './glyphs';
import { RouteChip } from './RouteChip';
import { ruleDescription } from './strings';
import { pill, StyledCard, StyledSection, StyledSectionTitle, StyledWarningCard } from './styles';

const StyledHeader = styled.div({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: '12px',
  marginBottom: '2px',
});

const StyledCount = styled.span({
  marginInlineStart: '6px',
  color: colors.whiteOnDarkBlue40,
  fontWeight: 600,
});

const StyledAddButton = styled.button({
  ...pill,
  gap: '6px',
  minHeight: '40px',
  padding: '0 14px',
  background: 'transparent',
  cursor: 'default',
  '&&:hover': {
    backgroundColor: colors.blue40,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '2px',
  },
});

const StyledList = styled.ul({
  listStyle: 'none',
  margin: 0,
  padding: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: '8px',
});

// The status line runs under the name and the chip both: a long route and a
// long status in a long language would not fit side by side on one line.
const StyledRuleButton = styled.button({
  ...sourceSansPro,
  display: 'grid',
  gridTemplateColumns: 'auto minmax(80px, 1fr) minmax(0, max-content)',
  alignItems: 'center',
  columnGap: '12px',
  rowGap: '2px',
  width: '100%',
  minHeight: '60px',
  padding: '10px 12px',
  border: 'none',
  borderRadius: '10px',
  backgroundColor: colors.blue40,
  color: colors.white,
  fontWeight: 400,
  textAlign: 'start',
  cursor: 'default',
  '&&:hover': {
    backgroundColor: colors.blue50,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '1px',
  },
});

const StyledAvatarCell = styled.span({
  gridRow: '1 / -1',
  display: 'flex',
});

const StyledRuleName = styled.span({
  gridColumn: 2,
  fontSize: '17px',
  lineHeight: '22px',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
});

const StyledChipCell = styled.span({
  gridColumn: 3,
  gridRow: 1,
  display: 'flex',
  justifyContent: 'flex-end',
  minWidth: 0,
});

const StyledCaveat = styled.span({
  display: 'block',
  fontSize: '13px',
  lineHeight: '17px',
  color: colors.nose,
});

const StyledStatusCell = styled.span({
  gridColumn: '2 / 4',
  minWidth: 0,
});

function AddIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
      aria-hidden>
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

type RuleRowProps = {
  target: RoutingTarget;
  icon?: string;
  route: AppRoute;
  exitLabel?: string;
  locked: boolean;
  onOpen: (target: RoutingTarget) => void;
  children?: React.ReactNode;
};

function RuleRow({ target, icon, route, exitLabel, locked, onOpen, children }: RuleRowProps) {
  const open = React.useCallback(() => onOpen(target), [onOpen, target]);
  return (
    <li>
      <StyledRuleButton
        type="button"
        aria-label={sprintf(
          // TRANSLATORS: Accessibility label of a rule in App routing, which
          // TRANSLATORS: opens the choice of its route. Available placeholders:
          // TRANSLATORS: %(application)s - the app's name
          // TRANSLATORS: %(route)s - its route: "Through the VPN", "Outside
          // TRANSLATORS: the VPN" or the country it leaves from
          messages.pgettext('split-tunneling-view', '%(application)s, %(route)s'),
          { application: target.name, route: ruleDescription(route, exitLabel, locked) },
        )}
        onClick={open}>
        <StyledAvatarCell>
          <AppAvatar name={target.name} icon={icon} />
        </StyledAvatarCell>
        <StyledRuleName>{target.name}</StyledRuleName>
        <StyledChipCell>
          <RouteChip route={route} exitLabel={exitLabel} locked={locked} />
        </StyledChipCell>
        {children && <StyledStatusCell>{children}</StyledStatusCell>}
      </StyledRuleButton>
    </li>
  );
}

// "Rules per app": every app whose route differs from the default, by name.
export function AppRulesSection() {
  const { routing, statuses, applications: metadata, platform } = useAppRouting();
  const { catalog, showAdd, showRoute } = useSplitTunnelingContext();
  const exitNames = useExitChoiceNames();
  const tunnelConnected = useSelector((state) => state.connection.status.state === 'connected');
  const rules = appRules(routing, platform);
  const direct = defaultRoute(routing) === 'direct';
  const linuxLaunchOnly = direct && platform === 'linux';

  const rows = React.useMemo(() => {
    const resolved = resolveApplications(
      rules.map((rule) => rule.app),
      metadata,
      catalog ?? [],
      platform,
    );
    return rules
      .map((rule, index) => ({ ...rule, application: resolved[index] }))
      .sort((a, b) => a.application.name.localeCompare(b.application.name));
  }, [catalog, metadata, platform, rules]);

  return (
    <StyledSection>
      <StyledHeader>
        <StyledSectionTitle>
          {
            // TRANSLATORS: Title of the list of apps that each have their own
            // TRANSLATORS: route, followed by their number.
            messages.pgettext('split-tunneling-view', 'Rules per app')
          }
          {rows.length > 0 && <StyledCount>{rows.length}</StyledCount>}
        </StyledSectionTitle>
        <StyledAddButton
          type="button"
          aria-label={messages.pgettext('split-tunneling-view', 'Add an app')}
          onClick={showAdd}>
          <AddIcon />
          {
            // TRANSLATORS: Short button adding an app to the rules, after a
            // TRANSLATORS: "+" sign. Keep it as short as possible.
            messages.pgettext('split-tunneling-view', 'App')
          }
        </StyledAddButton>
      </StyledHeader>

      {rows.length === 0 &&
        (direct && platform === 'linux' ? (
          // Linux keeps no list: an app joins the VPN when Warren opens it,
          // so no rule does not mean that no app uses the VPN.
          <StyledCard>
            {messages.pgettext(
              'split-tunneling-view',
              'No rules. Add an app to open it through the VPN: it uses the VPN until you close it.',
            )}
          </StyledCard>
        ) : direct ? (
          <StyledWarningCard role="note">
            <WarningGlyph />
            <span>
              {messages.pgettext(
                'split-tunneling-view',
                'No app uses the VPN. Add one, or send the other apps through the VPN again.',
              )}
            </span>
          </StyledWarningCard>
        ) : (
          <StyledCard>
            {messages.pgettext(
              'split-tunneling-view',
              'No rules. Add an app to send it through another country or outside the VPN.',
            )}
          </StyledCard>
        ))}

      {rows.length > 0 && (
        <StyledList data-testid="app-rules">
          {rows.map(({ app, route, locked, application }) => {
            const names = route.kind === 'country' ? exitNames(route.exit) : undefined;
            const line = ruleLine(routing, statuses, app, platform, tunnelConnected);
            return (
              <RuleRow
                key={app}
                target={{ id: app, name: application.name, application }}
                icon={application.icon}
                route={route}
                exitLabel={names ? (names.city ?? names.country) : undefined}
                locked={locked === true}
                onOpen={showRoute}>
                {line && <RouteStatusLine line={line} />}
                {route.kind === 'country' && linuxLaunchOnly && (
                  // Linux keeps no list: the country only applies to the app
                  // opened from Warren, which the row must not hide.
                  <StyledCaveat>
                    {messages.pgettext(
                      'split-tunneling-view',
                      'It uses this country when you open it with “Open through the VPN”.',
                    )}
                  </StyledCaveat>
                )}
              </RuleRow>
            );
          })}
        </StyledList>
      )}
    </StyledSection>
  );
}
