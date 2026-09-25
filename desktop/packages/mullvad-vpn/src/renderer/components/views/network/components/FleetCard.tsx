import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import { formatBytes, formatPercent, NetworkStats } from '../../../../../shared/network-stats';
import { colors, fleetLoadRingColor, spacings } from '../../../../lib/foundations';
import { useTweenedNumber } from '../../../../lib/network-stats';
import { useSelector } from '../../../../redux/store';
import { LiveIndicator, LoadRing, Sparkline, ThroughputPair } from '../../../network-stats';
import {
  StyledCard,
  StyledCardHeader,
  StyledFigures,
  StyledLabel,
  StyledRingCaption,
  StyledRingValue,
  StyledValue,
} from './styles';

const StyledHero = styled.div({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: spacings.medium,
});

const StyledCount = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
});

const StyledBigNumber = styled.span({
  color: colors.white,
  fontFamily: 'var(--font-family-source-sans-pro)',
  fontSize: '44px',
  fontWeight: 700,
  lineHeight: '46px',
  fontVariantNumeric: 'tabular-nums',
});

const StyledCharts = styled.div({
  display: 'grid',
  gridTemplateColumns: '1fr 1fr',
  gap: spacings.medium,
});

const StyledChart = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: '4px',
  minWidth: 0,
});

export type FleetCardProps = {
  stats: NetworkStats;
  stale: boolean;
};

export function FleetCard({ stats, stale }: FleetCardProps) {
  const locale = useSelector((state) => state.userInterface.locale);
  const connected = Math.round(useTweenedNumber(stats.users.connected));
  const load = useTweenedNumber(stats.fleet.loadPercent);
  const count = (value: number) => new Intl.NumberFormat(locale).format(value);

  return (
    <StyledCard $muted={stale} data-testid="network-fleet">
      <StyledCardHeader>
        <StyledLabel>
          {
            // TRANSLATORS: Label of the exact number of people connected to Warren.
            messages.pgettext('network-stats', 'People connected now')
          }
        </StyledLabel>
        <LiveIndicator stats={stats} stale={stale} />
      </StyledCardHeader>

      <StyledHero>
        <StyledCount>
          <StyledBigNumber data-testid="network-connected">{count(connected)}</StyledBigNumber>
        </StyledCount>
        <LoadRing
          size="large"
          level="unknown"
          color={fleetLoadRingColor}
          percent={stats.fleet.loadPercent}
          aria-label={sprintf(
            // TRANSLATORS: Accessibility label of the load of the whole Warren network.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(load)s - the load, e.g. "23%"
            messages.pgettext('network-stats', 'Network load %(load)s'),
            { load: formatPercent(stats.fleet.loadPercent, locale) },
          )}>
          <StyledRingValue aria-hidden>{formatPercent(Math.round(load), locale)}</StyledRingValue>
          <StyledRingCaption aria-hidden>
            {
              // TRANSLATORS: Caption under the load percentage of the whole network.
              messages.pgettext('network-stats', 'network load')
            }
          </StyledRingCaption>
        </LoadRing>
      </StyledHero>

      <StyledFigures>
        <StyledLabel as="dt">
          {
            // TRANSLATORS: Wallets registered on Warren, expired or not.
            messages.pgettext('network-stats', 'Accounts')
          }
        </StyledLabel>
        <StyledValue as="dd">{count(stats.users.accountsTotal)}</StyledValue>
        <StyledLabel as="dt">
          {
            // TRANSLATORS: Wallets with subscription time left.
            messages.pgettext('network-stats', 'Active subscribers')
          }
        </StyledLabel>
        <StyledValue as="dd">{count(stats.users.subscribersActive)}</StyledValue>
        <StyledLabel as="dt">
          {
            // TRANSLATORS: Warren exits serving now.
            messages.pgettext('network-stats', 'Exits online')
          }
        </StyledLabel>
        <StyledValue as="dd">
          {sprintf(
            // TRANSLATORS: Exits online out of all exits, e.g. "2 of 3".
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(online)d - exits serving now
            // TRANSLATORS: %(total)d - exits known to the network
            messages.pgettext('network-stats', '%(online)d of %(total)d'),
            { online: stats.fleet.exitsOnline, total: stats.fleet.exitsTotal },
          )}
        </StyledValue>
        <StyledLabel as="dt">
          {
            // TRANSLATORS: Traffic through the whole network right now.
            messages.pgettext('network-stats', 'Throughput')
          }
        </StyledLabel>
        <dd>
          <ThroughputPair
            downloadBps={stats.fleet.downloadBps}
            uploadBps={stats.fleet.uploadBps}
            locale={locale}
          />
        </dd>
        <StyledLabel as="dt">
          {
            // TRANSLATORS: Bytes carried for users over the last 24 hours.
            messages.pgettext('network-stats', 'Data carried, 24 h')
          }
        </StyledLabel>
        <StyledValue as="dd">{formatBytes(stats.fleet.transferred24hBytes, locale)}</StyledValue>
      </StyledFigures>

      {stats.history.length > 0 && (
        <StyledCharts>
          <StyledChart>
            <StyledLabel>
              {
                // TRANSLATORS: Caption of the chart of people connected over 24 hours.
                messages.pgettext('network-stats', 'People, 24 h')
              }
            </StyledLabel>
            <Sparkline
              values={stats.history.map((point) => point.connected)}
              width={140}
              height={36}
            />
          </StyledChart>
          <StyledChart>
            <StyledLabel>
              {
                // TRANSLATORS: Caption of the chart of network throughput over 24 hours.
                messages.pgettext('network-stats', 'Throughput, 24 h')
              }
            </StyledLabel>
            <Sparkline
              values={stats.history.map((point) => point.throughputBps)}
              width={140}
              height={36}
            />
          </StyledChart>
        </StyledCharts>
      )}
    </StyledCard>
  );
}
