import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import {
  appRouteFor,
  buildCountryOptions,
  type CountryOption,
  exitChoicesInUse,
  sameExitChoice,
} from '../../../../../../shared/app-routing';
import { type ExitChoice } from '../../../../../../shared/daemon-rpc-types';
import {
  messages,
  relayLocations as relayLocationsCatalog,
} from '../../../../../../shared/gettext';
import { CountryFlag } from '../../../../../features/app-routing/components';
import { useAppRouting } from '../../../../../features/app-routing/hooks';
import { IconButton } from '../../../../../lib/components';
import { colors } from '../../../../../lib/foundations';
import { useSelector } from '../../../../../redux/store';
import SearchBar from '../../../../SearchBar';
import { useRoutingActions } from '../../hooks/use-routing-actions';
import { useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { CheckBadge } from './glyphs';
import { ScreenFrame } from './ScreenFrame';
import {
  StyledHelp,
  StyledRowButton,
  StyledRowName,
  StyledScreenBody,
  StyledScreenButton,
  StyledScreenTitle,
} from './styles';

const StyledList = styled.ul({
  listStyle: 'none',
  margin: 0,
  padding: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: '8px',
});

const StyledCountry = styled.li({
  display: 'flex',
  flexDirection: 'column',
  gap: '8px',
});

const StyledCountryRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: '4px',
});

const StyledCityButton = styled(StyledRowButton)({
  minHeight: '48px',
  paddingInlineStart: '50px',
});

const StyledTag = styled.span({
  flexShrink: 0,
  fontSize: '13px',
  color: colors.whiteAlpha60,
});

const StyledEmpty = styled(StyledHelp)({
  textAlign: 'center',
  padding: '12px 0',
});

type ExitButtonProps = {
  exit: ExitChoice;
  label: string;
  current?: ExitChoice;
  exitsInUse: ExitChoice[];
  city?: boolean;
  onSelect: (exit: ExitChoice) => void;
};

function ExitButton({ exit, label, current, exitsInUse, city, onSelect }: ExitButtonProps) {
  const selected = current !== undefined && sameExitChoice(current, exit);
  const inUse = !selected && exitsInUse.some((used) => sameExitChoice(used, exit));
  const select = React.useCallback(() => onSelect(exit), [exit, onSelect]);
  const Row = city ? StyledCityButton : StyledRowButton;

  return (
    <Row type="button" $selected={selected} aria-pressed={selected} onClick={select}>
      {!city && <CountryFlag country={exit.country} size={22} />}
      <StyledRowName>{label}</StyledRowName>
      {inUse && (
        <StyledTag>
          {
            // TRANSLATORS: Tag on a country another app already leaves from.
            messages.pgettext('split-tunneling-view', 'In use')
          }
        </StyledTag>
      )}
      {selected && <CheckBadge />}
    </Row>
  );
}

type CountryItemProps = {
  option: CountryOption;
  current?: ExitChoice;
  exitsInUse: ExitChoice[];
  expanded: boolean;
  onToggle: (country: string) => void;
  onSelect: (exit: ExitChoice) => void;
};

function CountryItem({
  option,
  current,
  exitsInUse,
  expanded,
  onToggle,
  onSelect,
}: CountryItemProps) {
  const showCities = option.cities.length > 1;
  const citiesId = React.useId();
  const toggle = React.useCallback(() => onToggle(option.country), [onToggle, option.country]);

  return (
    <StyledCountry>
      <StyledCountryRow>
        <ExitButton
          exit={{ country: option.country }}
          label={option.name}
          current={current}
          exitsInUse={exitsInUse}
          onSelect={onSelect}
        />
        {showCities && (
          <IconButton
            variant="secondary"
            aria-expanded={expanded}
            aria-controls={citiesId}
            aria-label={sprintf(
              // TRANSLATORS: Accessibility label of the button listing the
              // TRANSLATORS: cities of a country. Available placeholders:
              // TRANSLATORS: %(country)s - the country's name
              messages.pgettext('split-tunneling-view', 'Cities in %(country)s'),
              { country: option.name },
            )}
            onClick={toggle}>
            <IconButton.Icon icon={expanded ? 'chevron-up' : 'chevron-down'} />
          </IconButton>
        )}
      </StyledCountryRow>
      {showCities && expanded && (
        <StyledList id={citiesId}>
          {option.cities.map((city) => (
            <li key={city.code}>
              <ExitButton
                exit={{ country: option.country, city: city.code }}
                label={city.name}
                current={current}
                exitsInUse={exitsInUse}
                city
                onSelect={onSelect}
              />
            </li>
          ))}
        </StyledList>
      )}
    </StyledCountry>
  );
}

// The country, and optionally the city, one app leaves from. Choosing gives
// the app its rule at once and goes back to its route; the main connection
// never moves.
export function CountryScreen() {
  const { routing, platform } = useAppRouting();
  const { target, showRoute } = useSplitTunnelingContext();
  const { setAppRoute } = useRoutingActions();
  const relayLocations = useSelector((state) => state.settings.relayLocations);
  const [searchTerm, setSearchTerm] = React.useState('');

  const route = target ? appRouteFor(routing, target.id, platform) : undefined;
  const current = route?.kind === 'country' ? route.exit : undefined;
  // Opens on the app's own country.
  const [expanded, setExpanded] = React.useState<ReadonlySet<string>>(
    () => new Set<string>(current ? [current.country] : []),
  );

  const translate = React.useCallback((name: string) => relayLocationsCatalog.gettext(name), []);
  const options = React.useMemo(
    () => buildCountryOptions(relayLocations, searchTerm, translate),
    [relayLocations, searchTerm, translate],
  );
  const exitsInUse = React.useMemo(
    () => exitChoicesInUse(routing.appExitsEnabled ? routing.appExits : []),
    [routing.appExits, routing.appExitsEnabled],
  );

  const back = React.useCallback(() => showRoute(), [showRoute]);
  const toggle = React.useCallback(
    (country: string) =>
      setExpanded((value) => {
        const next = new Set(value);
        if (!next.delete(country)) {
          next.add(country);
        }
        return next;
      }),
    [],
  );

  // A second pick before the daemon answers would plan from a stale state.
  const busy = React.useRef(false);
  const select = React.useCallback(
    async (exit: ExitChoice) => {
      if (target === undefined || busy.current) return;
      busy.current = true;
      await setAppRoute(target, { kind: 'country', exit });
      busy.current = false;
      showRoute();
    },
    [setAppRoute, showRoute, target],
  );

  if (target === undefined) {
    return null;
  }

  return (
    <ScreenFrame testId="country-screen" onClose={back}>
      <StyledScreenTitle>
        {sprintf(
          // TRANSLATORS: Title of the screen choosing the country one app
          // TRANSLATORS: leaves from. Available placeholders:
          // TRANSLATORS: %(application)s - the app's name
          messages.pgettext('split-tunneling-view', 'Country for %(application)s'),
          { application: target.name },
        )}
      </StyledScreenTitle>
      <SearchBar searchTerm={searchTerm} onSearch={setSearchTerm} />
      <StyledScreenBody>
        {options.length === 0 ? (
          <StyledEmpty>{messages.gettext('Try a different search.')}</StyledEmpty>
        ) : (
          <StyledList aria-label={messages.pgettext('split-tunneling-view', 'Countries')}>
            {options.map((option) => (
              <CountryItem
                key={option.country}
                option={option}
                current={current}
                exitsInUse={exitsInUse}
                // A search that only matched cities shows them straight away.
                expanded={expanded.has(option.country) || searchTerm !== ''}
                onToggle={toggle}
                onSelect={select}
              />
            ))}
          </StyledList>
        )}
      </StyledScreenBody>
      <StyledScreenButton type="button" onClick={back}>
        {messages.gettext('Cancel')}
      </StyledScreenButton>
    </ScreenFrame>
  );
}
