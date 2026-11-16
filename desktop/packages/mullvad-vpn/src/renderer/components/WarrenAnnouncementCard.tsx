import { useCallback, useEffect, useRef, useState } from 'react';
import styled from 'styled-components';

import { Url } from '../../shared/constants';
import { messages } from '../../shared/gettext';
import { InAppAnnouncementCard } from '../../shared/notifications/notification';
import { copyToClipboard } from '../lib/clipboard';
import { Button } from '../lib/components/button';
import { Icon } from '../lib/components/icon';
import { IconButton } from '../lib/components/icon-button';
import { Text } from '../lib/components/text';
import { homeTokens, Radius, spacings, surfaces } from '../lib/foundations';
import { NotificationSubtitleText, NotificationTitle } from './NotificationBanner';

// How long the copy confirmation stays up. Long enough to be read after the
// eye has moved back to the code, short enough that it never looks stuck.
const COPIED_FEEDBACK_MS = 2000;

const Card = styled.div({
  display: 'flex',
  flex: 1,
  // One column, always. The banner is capped at 300px, so anything laid out
  // side by side collapses into two unreadable columns at the first
  // translation that runs long.
  flexDirection: 'column',
  gap: spacings.small,
  minWidth: 0,
});

// The headline and the body read as one block, as in the banners.
const TextBlock = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: '1px',
  minWidth: 0,
});

const Header = styled.div({
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'flex-start',
  gap: spacings.small,
  minWidth: 0,
});

const Headline = styled(NotificationTitle)({
  flex: 1,
  minWidth: 0,
});

const Body = styled(NotificationSubtitleText)({});

// A bare cross, as the mockup puts an announcement away.
const StyledDismiss = styled.button({
  display: 'grid',
  placeItems: 'center',
  flexShrink: 0,
  width: '24px',
  height: '24px',
  margin: '-3px -4px 0 0',
  padding: 0,
  border: 'none',
  borderRadius: '6px',
  background: 'none',
  color: surfaces.homeNoticeTextSecondary,
  cursor: 'pointer',
  '&:hover': { color: surfaces.homeText },
  '&:focus-visible': { outline: `2px solid ${surfaces.homeText}`, outlineOffset: '-2px' },
});

const StyledDismissIcon = styled(Icon)({
  '&&': {
    width: `${homeTokens.notice.close}px`,
    height: `${homeTokens.notice.close}px`,
    backgroundColor: 'currentColor',
  },
});

// Inset well under the body, so the code reads as a field to act on rather
// than as more prose.
const VoucherWell = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: spacings.tiny,
  padding: `${spacings.small} ${spacings.small}`,
  borderRadius: Radius.radius8,
  border: `1px solid ${surfaces.homeToolbarLine}`,
  backgroundColor: surfaces.homeField,
});

const VoucherLabelRow = styled.div({
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: spacings.small,
});

const VoucherLabel = styled(Text)({
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
});

const VoucherRow = styled.div({
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  gap: spacings.small,
});

// `html { user-select: none }` is set globally, and nothing in the
// notification path lifts it, so a code rendered without this override cannot
// even be selected. The monospace face is not decoration either: it is what
// makes a 16 character code transcribable by eye.
const VoucherCode = styled.span({
  flex: 1,
  minWidth: 0,
  userSelect: 'text',
  cursor: 'text',
  fontFamily:
    '"JetBrains Mono", "SF Mono", "Cascadia Mono", "Roboto Mono", "Ubuntu Mono", monospace',
  fontSize: '14px',
  lineHeight: '20px',
  letterSpacing: '0.04em',
  color: surfaces.homeText,
  overflowWrap: 'anywhere',
});

// The gettext lookups run here rather than in the JSX ternary, where the
// extractor cannot attach the translator notes to either arm.
function voucherLabel(copied: boolean): string {
  if (copied) {
    // TRANSLATORS: Confirmation shown once the voucher code is on the clipboard.
    return messages.pgettext('in-app-notifications', 'Copied');
  }
  // TRANSLATORS: Label of the voucher code an announcement granted to this account.
  return messages.pgettext('in-app-notifications', 'Your code');
}

export interface WarrenAnnouncementCardProps {
  title: string;
  announcement: InAppAnnouncementCard;
  onOpenCta: (url: Url) => void;
}

export function WarrenAnnouncementCard({
  title,
  announcement,
  onOpenCta,
}: WarrenAnnouncementCardProps) {
  const { voucherCode, cta } = announcement;
  const [copied, setCopied] = useState(false);
  const resetTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(
    () => () => {
      if (resetTimer.current !== undefined) {
        clearTimeout(resetTimer.current);
      }
    },
    [],
  );

  const copyVoucher = useCallback(() => {
    if (voucherCode === null) {
      return;
    }
    void copyToClipboard(voucherCode).then((copiedNow) => {
      if (!copiedNow) {
        return;
      }
      setCopied(true);
      if (resetTimer.current !== undefined) {
        clearTimeout(resetTimer.current);
      }
      resetTimer.current = setTimeout(() => setCopied(false), COPIED_FEEDBACK_MS);
    });
  }, [voucherCode]);

  const openCta = useCallback(() => {
    if (cta !== null) {
      onOpenCta(cta.url);
    }
  }, [cta, onOpenCta]);

  return (
    <Card data-testid="announcementCard">
      <TextBlock>
        <Header>
          <Headline data-testid="announcementHeadline">{title}</Headline>
          <StyledDismiss
            onClick={announcement.dismiss}
            data-testid="announcementDismiss"
            aria-label={
              // TRANSLATORS: Accessibility label of the control that puts an
              // TRANSLATORS: announcement away for good.
              messages.pgettext('accessibility', 'Dismiss the announcement')
            }>
            <StyledDismissIcon icon="cross" />
          </StyledDismiss>
        </Header>

        <Body data-testid="announcementBody">{announcement.body}</Body>
      </TextBlock>

      {voucherCode !== null && (
        <VoucherWell>
          <VoucherLabelRow>
            <VoucherLabel
              variant="footnoteMiniSemiBold"
              color={copied ? 'greenText' : 'whiteAlpha60'}>
              {voucherLabel(copied)}
            </VoucherLabel>
          </VoucherLabelRow>
          <VoucherRow>
            <VoucherCode data-testid="announcementVoucherCode">{voucherCode}</VoucherCode>
            {copied ? (
              <Icon icon="checkmark" size="small" color="green" />
            ) : (
              <IconButton
                size="small"
                onClick={copyVoucher}
                data-testid="announcementCopyVoucher"
                aria-label={
                  // TRANSLATORS: Accessibility label of the control that copies
                  // TRANSLATORS: the voucher code to the clipboard.
                  messages.pgettext('accessibility', 'Copy the voucher code')
                }>
                <IconButton.Icon icon="copy" />
              </IconButton>
            )}
          </VoucherRow>
        </VoucherWell>
      )}

      {cta !== null && (
        <Button variant="success" onClick={openCta} data-testid="announcementCta">
          <Button.Text>{cta.label}</Button.Text>
          <Button.Icon icon="external" />
        </Button>
      )}
    </Card>
  );
}
