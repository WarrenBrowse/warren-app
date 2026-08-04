import { ExternalLinkProps } from '../../renderer/components/ExternalLink';
import { InternalLinkProps } from '../../renderer/components/InternalLink';
import { ButtonProps } from '../../renderer/lib/components';
import { RoutePath } from '../../shared/routes';
import { Url } from '../constants';

export type SystemNotificationAction =
  | {
      type: 'navigate-internal';
      link: {
        to: RoutePath;
        text?: string;
      };
    }
  | {
      type: 'navigate-external';
      link: {
        to: Url;
        text?: string;
        withAuth?: boolean;
      };
    };

export interface InAppNotificationTroubleshootInfo {
  details: string;
  steps: string[];
  buttons?: Array<InAppNotificationTroubleshootButton>;
}

export interface InAppNotificationTroubleshootButton {
  label: string;
  action: () => void;
  variant?: 'primary' | 'success' | 'destructive';
}

export type InAppNotificationAction =
  | {
      type: 'troubleshoot-dialog';
      troubleshoot: InAppNotificationTroubleshootInfo;
    }
  | {
      type: 'close';
      close: () => void;
    }
  | {
      type: 'navigate-internal';
      link: Pick<InternalLinkProps, 'to' | 'onClick' | 'aria-label'>;
    }
  | {
      type: 'navigate-external';
      link: Pick<ExternalLinkProps, 'to' | 'onClick' | 'aria-label' | 'withAuth'>;
    }
  | {
      type: 'run-function';
      button: Pick<ButtonProps, 'onClick' | 'aria-label'>;
    }
  | {
      // The banner text is unbounded (an operator-authored notice), so it is
      // clamped to a few lines and the full text moves to a scrollable modal.
      // Carrying the content here rather than reusing the subtitle keeps the
      // modal independent of how the subtitle happens to be rendered.
      type: 'expand-text';
      expand: { title: string; content: string };
    };

export type InAppNotificationIndicatorType = 'success' | 'warning' | 'error';

export enum SystemNotificationSeverityType {
  info = 0,
  low,
  medium,
  high,
}

export enum SystemNotificationCategory {
  tunnelState,
  expiry,
  newVersion,
  inconsistentVersion,
  // Auto-renewal lifecycle (warren-core doc 65): its own category so device-event
  // handling that clears `expiry` notifications leaves these alone.
  renewal,
}

interface NotificationProvider {
  mayDisplay(): boolean;
}

export interface SystemNotification {
  message: string;
  severity: SystemNotificationSeverityType;
  category: SystemNotificationCategory;
  throttle?: boolean;
  presentOnce?: { value: boolean; name: string };
  suppressInDevelopment?: boolean;
  action?: SystemNotificationAction;
}

export interface InAppNotification {
  indicator?: InAppNotificationIndicatorType;
  action?: InAppNotificationAction;
  title: string;
  subtitle?: string | React.ReactElement | InAppNotificationSubtitle[];
}

export type InAppNotificationSubtitleString = {
  content: string;
};

export type InAppNotificationSubtitleElement = {
  content: React.ReactElement;
  key: string;
};

export type InAppNotificationSubtitle = (
  | InAppNotificationSubtitleString
  | InAppNotificationSubtitleElement
) & {
  action?: InAppNotificationAction;
};

export interface SystemNotificationProvider extends NotificationProvider {
  getSystemNotification(): SystemNotification | undefined;
}

export interface InAppNotificationProvider extends NotificationProvider {
  getInAppNotification(): InAppNotification | undefined;
}
