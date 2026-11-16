import { motion } from 'motion/react';
import React, { useLayoutEffect, useRef, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../shared/gettext';
import {
  InAppNotificationIcon,
  InAppNotificationIndicatorType,
} from '../../shared/notifications/notification';
import { Icon, IconButton, IconProps } from '../lib/components';
import { colors, surfaces } from '../lib/foundations';
import { useExclusiveTask } from '../lib/hooks/use-exclusive-task';
import { smallText, tinyText } from './common-styles';

const NOTIFICATION_AREA_ID = 'notification-area';

// The banner carries operator-authored text, so the title and the body have to
// separate at a glance rather than being one block of bold 12px. Same hierarchy
// the announcement card uses: a title one step up the size ladder, and a body
// at regular weight with enough contrast to survive the glass surface over the
// scenery photo.
export const NotificationTitle = styled.span(smallText, {
  fontSize: '13px',
  lineHeight: '17px',
  fontWeight: 600,
  color: surfaces.homeText,
  overflowWrap: 'anywhere',
});

export const NotificationSubtitleText = styled.span(tinyText, {
  fontSize: '11px',
  lineHeight: '15px',
  fontWeight: 'normal',
  color: surfaces.homeTextSecondary,
  overflowWrap: 'anywhere',
});

interface INotificationSubtitleProps {
  children?: React.ReactNode;
}

export function NotificationSubtitle(props: INotificationSubtitleProps) {
  return React.Children.count(props.children) > 0 ? <NotificationSubtitleText {...props} /> : null;
}

// How many lines of an unbounded banner text are shown before it is clamped.
// Three keeps the banner from swallowing the connect view, which a long
// operator notice otherwise does.
const CLAMPED_SUBTITLE_LINES = 3;

const ClampedSubtitleText = styled(NotificationSubtitleText)<{ $clickable: boolean }>((props) => ({
  display: '-webkit-box',
  WebkitBoxOrient: 'vertical',
  WebkitLineClamp: CLAMPED_SUBTITLE_LINES,
  overflow: 'hidden',
  cursor: props.$clickable ? 'pointer' : 'auto',
}));

const ExpandHint = styled.button(tinyText, {
  alignSelf: 'flex-start',
  marginTop: '2px',
  padding: 0,
  border: 'none',
  background: 'none',
  color: colors.white,
  textDecoration: 'underline',
  cursor: 'pointer',
});

interface INotificationExpandableSubtitleProps {
  text: string;
  // Label of the affordance shown only when the text is actually clamped.
  expandLabel: string;
  onExpand: () => void;
}

// Subtitle for a banner whose text has no length bound. It renders clamped and
// says so: the affordance appears only when the text really overflows, which is
// measured rather than guessed from a character count, because the banner width
// changes with the window.
export function NotificationExpandableSubtitle(props: INotificationExpandableSubtitleProps) {
  const textRef = useRef<HTMLSpanElement>(null);
  const [truncated, setTruncated] = useState(false);

  useLayoutEffect(() => {
    const element = textRef.current;
    if (!element) {
      return;
    }
    // A one-pixel slack absorbs sub-pixel line-height rounding, which would
    // otherwise report a clamp on text that fits exactly.
    const measure = () => setTruncated(element.scrollHeight - element.clientHeight > 1);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [props.text]);

  return (
    <>
      <ClampedSubtitleText
        ref={textRef}
        $clickable={truncated}
        onClick={truncated ? props.onExpand : undefined}>
        {props.text}
      </ClampedSubtitleText>
      {truncated && (
        <ExpandHint onClick={props.onExpand} aria-describedby={NOTIFICATION_AREA_ID}>
          {props.expandLabel}
        </ExpandHint>
      )}
    </>
  );
}

interface NotificationActionProps {
  onClick: () => Promise<void>;
}

export function NotificationOpenLinkAction(props: NotificationActionProps) {
  const [onClick] = useExclusiveTask(props.onClick);
  return (
    <IconButton
      size="small"
      variant="secondary"
      onClick={onClick}
      aria-describedby={NOTIFICATION_AREA_ID}
      aria-label={messages.gettext('Open URL')}>
      <IconButton.Icon icon="external" />
    </IconButton>
  );
}

export function NotificationTroubleshootDialogAction(props: NotificationActionProps) {
  return (
    <IconButton
      size="small"
      variant="secondary"
      aria-describedby={NOTIFICATION_AREA_ID}
      aria-label={messages.gettext('Troubleshoot')}
      onClick={props.onClick}>
      <IconButton.Icon icon="info-circle" />
    </IconButton>
  );
}

// A bare cross, as the mockup puts away an announcement.
const StyledCloseButton = styled.button({
  display: 'grid',
  placeItems: 'center',
  width: '24px',
  height: '24px',
  padding: 0,
  border: 'none',
  borderRadius: '6px',
  background: 'none',
  color: surfaces.homeTextSecondary,
  cursor: 'pointer',
  '&:hover': { color: surfaces.homeText },
  '&:focus-visible': { outline: `2px solid ${surfaces.homeText}`, outlineOffset: '-2px' },
});

const StyledCloseIcon = styled(Icon)({
  '&&': { width: '16px', height: '16px', backgroundColor: 'currentColor' },
});

export function NotificationCloseAction(props: NotificationActionProps) {
  const [onClick] = useExclusiveTask(props.onClick);
  return (
    <StyledCloseButton
      aria-describedby={NOTIFICATION_AREA_ID}
      aria-label={messages.pgettext('accessibility', 'Close notification')}
      onClick={onClick}>
      <StyledCloseIcon icon="cross" />
    </StyledCloseButton>
  );
}

// The labelled button a banner ends with: the mockup's light "Install".
const StyledBannerButton = styled.button({
  flexShrink: 0,
  padding: '6px 10px',
  border: 'none',
  borderRadius: '8px',
  background: surfaces.homeText,
  color: surfaces.homePage,
  fontFamily: 'var(--font-family-sans)',
  fontSize: '12px',
  lineHeight: '16px',
  fontWeight: 600,
  whiteSpace: 'nowrap',
  cursor: 'pointer',
  '&:hover': { opacity: 0.9 },
  '&:focus-visible': { outline: `2px solid ${surfaces.homeText}`, outlineOffset: '2px' },
});

export function NotificationButton(props: NotificationActionProps & { label: string }) {
  const [onClick] = useExclusiveTask(props.onClick);
  return (
    <StyledBannerButton aria-describedby={NOTIFICATION_AREA_ID} onClick={onClick}>
      {props.label}
    </StyledBannerButton>
  );
}

const iconByKind: Record<InAppNotificationIcon, IconProps['icon']> = {
  update: 'download',
  announcement: 'megaphone',
};

const iconByLevel: Record<InAppNotificationIndicatorType, IconProps['icon']> = {
  success: 'info-circle',
  warning: 'alert-circle',
  error: 'alert-circle',
};

const StyledLeadIcon = styled(Icon)<{ $color: string }>((props) => ({
  '&&': { width: '16px', height: '16px', backgroundColor: props.$color },
  flexShrink: 0,
}));

// The glyph a banner leads with, in place of a coloured dot: the mockup's
// amber download for an update, a megaphone for an announcement.
export function NotificationLeadIcon(props: {
  kind?: InAppNotificationIcon;
  indicator?: InAppNotificationIndicatorType;
}) {
  const icon = props.kind ? iconByKind[props.kind] : iconByLevel[props.indicator ?? 'success'];
  const color =
    props.kind === 'announcement'
      ? surfaces.homeTextSecondary
      : props.indicator === 'error'
        ? surfaces.homeDisconnectText
        : props.indicator === 'success' && !props.kind
          ? surfaces.homeTextSecondary
          : surfaces.homeBadge;
  return (
    <StyledLeadIcon
      icon={icon}
      $color={color}
      aria-hidden
      data-testid="notificationIndicator"
      data-level={props.indicator}
    />
  );
}

export const NotificationContent = styled.div.attrs({ id: NOTIFICATION_AREA_ID })({
  display: 'flex',
  flexDirection: 'column',
  gap: '1px',
  flex: 1,
  minWidth: 0,
});

export const NotificationActions = styled.div({
  display: 'flex',
  flex: 0,
  alignItems: 'center',
  gap: '4px',
});

interface INotificationIndicatorProps {
  $type?: InAppNotificationIndicatorType;
}

const notificationIndicatorTypeColorMap = {
  success: colors.green,
  warning: colors.yellow,
  error: colors.red,
};

export const NotificationIndicator = styled.div<INotificationIndicatorProps>((props) => ({
  width: '10px',
  height: '10px',
  borderRadius: '5px',
  // Centred on the title's line box rather than on its cap height, so the dot
  // reads as part of the title line: (20px line height - 10px dot) / 2.
  marginTop: '5px',
  marginInlineEnd: '8px',
  backgroundColor: props.$type
    ? notificationIndicatorTypeColorMap[props.$type]
    : colors.transparent,
}));

// The mockup's banner: the toolbar's own glass, under it and on its right
// edge, as wide as its words need up to the toolbar's reach.
const Collapsible = styled(motion.div)({
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'flex-start',
  translateY: '0%',
  marginInlineStart: 'auto',
  maxWidth: '272px',
  borderRadius: '12px',
  backgroundColor: surfaces.homeToolbar,
  backdropFilter: 'blur(10px)',
  boxShadow: `inset 0 0 0 1px ${surfaces.homeToolbarLine}, 0 8px 24px -8px rgba(0, 0, 0, 0.45)`,
  overflow: 'hidden',
});

const Content = styled.section({
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  gap: '10px',
  padding: '10px 12px',
  height: 'fit-content',
});

interface INotificationBannerProps {
  children?: React.ReactNode; // Array<NotificationContent | NotificationActions>,
  className?: string;
  animateIn: boolean;
  indicator?: InAppNotificationIndicatorType;
}

export function NotificationBanner({
  className,
  children,
  animateIn,
  indicator,
}: INotificationBannerProps) {
  const translateYInitial = animateIn ? '-100%' : '0%';

  return (
    <Collapsible
      data-level={indicator}
      animate={{ translateY: '0%' }}
      className={className}
      exit={{ translateY: '-100%' }}
      initial={{ translateY: translateYInitial }}
      transition={{ duration: 0.25 }}>
      <Content>{children}</Content>
    </Collapsible>
  );
}
