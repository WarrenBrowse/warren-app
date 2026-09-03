import fs from 'fs';
import path from 'path';
import { renderToStaticMarkup } from 'react-dom/server';
import { ServerStyleSheet } from 'styled-components';

import { WarrenAnnouncementCard } from '../../src/renderer/components/WarrenAnnouncementCard';
import {
  colorPrimitives,
  fontFamilies,
  fontSizes,
  fontWeights,
  lineHeights,
  radius,
  spacingPrimitives,
} from '../../src/renderer/lib/foundations/variables';
import { WarrenAnnouncementNotificationProvider } from '../../src/renderer/lib/notifications';
import { WarrenAnnouncement } from '../../src/shared/daemon-rpc-types';

// A standalone page showing the announcement card as the connect screen draws
// it, so the design can be judged by eye without launching Electron:
//
//   WARREN_CARD_PREVIEW=1 npx vitest run test/unit/warren-announcement-card.spec.tsx
//   open build/announcement-card-preview.html
//
// It goes through the real provider, the real component, the app's own global
// stylesheet and the app's own fonts and icons, so a change to any of them
// shows up here without the preview being maintained on the side. Every asset
// is inlined, because a page that reaches for a sibling file renders with no
// fonts and no icons the moment it is opened from the filesystem.

const PACKAGE_ROOT = path.resolve(__dirname, '../..');

const SAMPLES: WarrenAnnouncement[] = [
  {
    id: 'prod-launch-2026',
    headline: 'Warren production is open',
    body: 'The production network is live, with the full exit fleet and no bandwidth cap. Your beta account keeps working exactly as it does today.',
    level: 'info',
    cta: { label: 'Get the production app', url: 'https://warren.ro/telecharger' },
    voucherCode: 'ABCD-EFGH-JKMN-PQRS',
  },
  {
    id: 'prod-launch-2026-no-code',
    headline: 'Warren production is open',
    body: 'The production network is live, with the full exit fleet and no bandwidth cap. Your beta account keeps working exactly as it does today.',
    level: 'info',
    cta: { label: 'Get the production app', url: 'https://warren.ro/telecharger' },
    voucherCode: null,
  },
  {
    id: 'beta-sunset',
    headline: 'This beta stops accepting new connections on 1 December',
    body: 'Move to the production app before then. Nothing on this account is lost, and the subscription follows you.',
    level: 'warning',
    cta: null,
    voucherCode: null,
  },
];

// The banner geometry of NotificationBanner's own Collapsible. Repeated here
// rather than rendered through it, because the real one is a motion component
// whose entry animation would freeze the preview mid-slide.
const BANNER_STYLE = [
  'display:flex',
  'flex-direction:row',
  'max-width:300px',
  'border-radius:14px',
  'border:1px solid var(--color-white-alpha20)',
  'border-top:2px solid var(--color-green)',
  'background-color:var(--color-black-alpha60)',
  'box-shadow:0 8px 24px rgba(0, 0, 0, 0.35)',
  'padding:10px 12px 10px 16px',
].join(';');

function readAsset(relative: string): string {
  return fs.readFileSync(path.join(PACKAGE_ROOT, relative), 'utf8');
}

function dataUri(relative: string, mime: string): string {
  const bytes = fs.readFileSync(path.join(PACKAGE_ROOT, relative));
  return `data:${mime};base64,${bytes.toString('base64')}`;
}

// The app's own @font-face block, with each file carried inside the page.
function fontFaces(): string {
  return readAsset('assets/css/fonts.css').replace(
    /url\('\.\.\/fonts\/([\w-]+\.ttf)'\)/g,
    (_match, file: string) => `url('${dataUri(path.join('assets/fonts', file), 'font/ttf')}')`,
  );
}

// The icon components mask an SVG by url. Nothing serves that path here.
function inlineIcons(css: string): string {
  return css.replace(
    /url\(assets\/icons\/([\w-]+\.svg)\)/g,
    (_match, file: string) => `url("${dataUri(path.join('assets/icons', file), 'image/svg+xml')}")`,
  );
}

function rootVariables(): string {
  return Object.entries({
    ...spacingPrimitives,
    ...colorPrimitives,
    ...radius,
    ...fontFamilies,
    ...fontSizes,
    ...fontWeights,
    ...lineHeights,
  })
    .map(([name, value]) => `  ${name}: ${value};`)
    .join('\n');
}

export function renderAnnouncementCardPreview(): string {
  const sheet = new ServerStyleSheet();
  const cards = SAMPLES.map((announcement) => {
    const notification = new WarrenAnnouncementNotificationProvider({
      announcements: [announcement],
      dismissedIds: [],
      dismiss: () => undefined,
    }).getInAppNotification();
    if (notification.action?.type !== 'announcement-card') {
      throw new Error('the announcement provider stopped producing a card');
    }
    return renderToStaticMarkup(
      sheet.collectStyles(
        <WarrenAnnouncementCard
          title={notification.title}
          indicator={notification.indicator}
          announcement={notification.action.announcement}
          onOpenCta={() => undefined}
        />,
      ),
    );
  });

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Warren announcement card</title>
<style>
${fontFaces()}
</style>
<style>
${readAsset('assets/css/reset.css')}
</style>
<style>
${readAsset('assets/css/global.css')}
</style>
<style>
:root {
${rootVariables()}
}
body {
  height: auto;
  padding: 40px;
  background: var(--color-dark-blue);
  display: flex;
  gap: 32px;
  align-items: flex-start;
}
</style>
${inlineIcons(sheet.getStyleTags())}
</head>
<body>
${cards.map((card) => `<div style="${BANNER_STYLE}">${card}</div>`).join('\n')}
</body>
</html>
`;
}
