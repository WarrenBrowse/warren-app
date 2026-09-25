import styled from 'styled-components';

import { messages } from '../../../shared/gettext';
import { formatBitsPerSecond } from '../../../shared/network-stats';
import { colors } from '../../lib/foundations';
import { ArrowGlyph } from './glyphs';

const StyledPair = styled.span({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '10px',
  color: colors.white,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '12px',
  fontWeight: 600,
  lineHeight: '18px',
  whiteSpace: 'nowrap',
  fontVariantNumeric: 'tabular-nums',
});

const StyledDirection = styled.span({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '3px',
  '& > svg': {
    color: colors.whiteAlpha60,
  },
});

export type ThroughputPairProps = {
  downloadBps: number;
  uploadBps: number;
  locale: string;
};

export function ThroughputPair({ downloadBps, uploadBps, locale }: ThroughputPairProps) {
  return (
    <StyledPair data-testid="throughput-pair">
      <StyledDirection
        aria-label={
          // TRANSLATORS: Accessibility label of the download rate of an exit.
          messages.pgettext('network-stats', 'Download')
        }>
        <ArrowGlyph direction="down" />
        {formatBitsPerSecond(downloadBps, locale)}
      </StyledDirection>
      <StyledDirection
        aria-label={
          // TRANSLATORS: Accessibility label of the upload rate of an exit.
          messages.pgettext('network-stats', 'Upload')
        }>
        <ArrowGlyph direction="up" />
        {formatBitsPerSecond(uploadBps, locale)}
      </StyledDirection>
    </StyledPair>
  );
}
