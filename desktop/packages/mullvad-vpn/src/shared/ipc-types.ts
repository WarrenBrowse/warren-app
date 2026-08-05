import { Action, Location } from 'history';

import { TransitionType } from '../renderer/lib/history';

export interface ICurrentAppVersionInfo {
  gui: string;
  daemon?: string;
  isConsistent: boolean;
  isBeta: boolean;
}

export interface IWindowShapeParameters {
  arrowPosition?: number;
}

export type ScrollToAnchorId =
  | 'daita-enable-setting'
  | 'multihop-setting'
  | 'custom-dns-settings'
  | 'allow-external-dns-setting'
  | 'allow-lan-setting'
  | 'lockdown-mode-setting'
  | 'dns-blocker-setting'
  | 'mtu-setting'
  // Anchor for the client-side bandwidth ceiling input.
  | 'max-rate-setting'
  | 'obfuscation-setting'
  | 'port-setting'
  | 'mss-fix-setting'
  | 'quantum-resistant-setting'
  // Anchor for the warren-api URL input.
  | 'warren-api-url-setting'
  // Anchors for the Warren multi-hop view.
  | 'warren-multi-hop-setting'
  | 'warren-multi-hop-entry-country'
  | 'warren-multi-hop-exit-country'
  // Anchors for the Warren status display (reconnect_count + age,
  // obfuscation indicator).
  | 'warren-status-reconnect'
  | 'warren-obfuscation-indicator'
  // Anchor for the port-forwarding toggle row.
  | 'port-forwarding-setting'
  // Anchor for the port-forwarding advanced controls (protocol +
  // preferred port). Used when an external link (e.g., a "Configure
  // your port" CTA in a future onboarding step) wants to deep-link
  // directly to the form rather than the toggle.
  | 'port-forwarding-advanced';

export type ScrollToAnchorOption = {
  type: 'scroll-to-anchor';
  id: ScrollToAnchorId;
};

export type LocationStateOptions = ScrollToAnchorOption;

/**
 * A run of inline text inside a changelog block. The parser resolves Markdown
 * emphasis in the main process so the renderer never interprets markup, which
 * keeps the release notes off any HTML-injection path.
 */
export type ChangelogInline =
  | { type: 'text'; value: string }
  | { type: 'strong'; value: string }
  | { type: 'code'; value: string }
  | { type: 'link'; value: string; href: string };

export type ChangelogBlock =
  | { type: 'heading'; level: number; content: ChangelogInline[] }
  | { type: 'paragraph'; content: ChangelogInline[] }
  | { type: 'list'; items: ChangelogInline[][] };

/**
 * Parsed release notes. Still an array, so the "is there anything to show"
 * checks that gate the changelog views keep working unchanged.
 */
export type IChangelog = Array<ChangelogBlock>;

export interface LocationState {
  scrollPosition: [number, number];
  expandedSections: Record<string, boolean>;
  transition: TransitionType;
  options?: LocationStateOptions[];
}

export interface IHistoryObject {
  entries: Location<LocationState>[];
  index: number;
  lastAction: Action;
}

export type ScrollPositions = Record<string, [number, number]>;

export type DaemonStatus = 'start-requested' | 'running' | 'stopped';
