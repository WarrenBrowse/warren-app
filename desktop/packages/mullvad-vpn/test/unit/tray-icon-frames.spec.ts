import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import {
  TRAY_ICON_FRAME_COUNT,
  trayIconFileName,
  trayIconTargetFrame,
} from '../../src/main/tray-icon-controller';
import { coverage, pngSize } from './png-pixels';

const ASSETS_DIR = path.resolve(__dirname, '../../assets/images/menubar-icons');
const TREES = ['', 'beta'];

const read = (platform: string, name: string, tree = '') =>
  fs.readFileSync(path.join(ASSETS_DIR, platform, tree, name));

describe('the tray icon animation', () => {
  it('passes through the securing frame on its way from unsecured to secured', () => {
    // The tray steps one frame at a time towards its target, so connecting
    // shows the fill rising to the blocked state before the tunnel is up, and
    // disconnecting drains it the same way back.
    const unsecured = trayIconTargetFrame('unsecured');
    const securing = trayIconTargetFrame('securing');
    const secured = trayIconTargetFrame('secured');

    expect(unsecured).toBe(0);
    expect(securing).toBeGreaterThan(unsecured);
    expect(secured).toBeGreaterThan(securing);
    expect(secured).toBe(TRAY_ICON_FRAME_COUNT - 1);
  });

  it('names its frames from one, the way the asset trees do', () => {
    expect(trayIconFileName(0, '')).toBe('tray-1');
    expect(trayIconFileName(TRAY_ICON_FRAME_COUNT - 1, 'Template')).toBe(
      `tray-${TRAY_ICON_FRAME_COUNT}Template`,
    );
  });
});

describe('the monochrome tray icon', () => {
  // A template image loses its colour, so the state has to live in the shape:
  // the inside of the W fills as the tunnel comes up.
  const templates = (tree: string) =>
    Array.from({ length: TRAY_ICON_FRAME_COUNT }, (_, frame) =>
      coverage(read('darwin', `${trayIconFileName(frame, 'Template')}@2x.png`, tree)),
    );

  it('fills a little more at every frame of the animation', () => {
    for (const tree of TREES) {
      const painted = templates(tree);
      for (let frame = 1; frame < painted.length; frame++) {
        expect(painted[frame], `${tree} frame ${frame + 1}`).toBeGreaterThan(painted[frame - 1]);
      }
    }
  });

  it('tells the three tunnel states apart by how much of the W is filled', () => {
    const painted = templates('');
    const unsecured = painted[trayIconTargetFrame('unsecured')];
    const securing = painted[trayIconTargetFrame('securing')];
    const secured = painted[trayIconTargetFrame('secured')];

    // Wide margins, so a state reads at a glance and not only in a diff.
    expect(securing).toBeGreaterThan(unsecured * 1.4);
    expect(secured).toBeGreaterThan(securing * 1.3);
  });
});

describe('the tray notification dot', () => {
  it('sits inside the icon instead of widening it', () => {
    // The dot lives between the ears, so the menu bar item keeps its width and
    // the neighbouring icons do not shift when a notification comes in.
    const pairs = [
      ['darwin', 'tray-1.png', 'tray-1_notification.png'],
      ['darwin', 'tray-1@2x.png', 'tray-1_notification@2x.png'],
      ['darwin', 'tray-1Template.png', 'tray-1_notificationTemplate.png'],
      ['darwin', 'tray-1Template@2x.png', 'tray-1_notificationTemplate@2x.png'],
      ['linux', 'tray-1.png', 'tray-1_notification.png'],
      ['linux', 'tray-1_white.png', 'tray-1_white_notification.png'],
    ];
    for (const tree of TREES) {
      for (const [platform, plain, dotted] of pairs) {
        expect(pngSize(read(platform, dotted, tree)), `${platform}/${tree}/${dotted}`).toEqual(
          pngSize(read(platform, plain, tree)),
        );
      }
    }
  });

  it('paints the icon, and not only the dot, in every notification variant', () => {
    // Guards the knockout ring around the dot: a mask that swallowed the mark
    // would leave a lone dot in the menu bar.
    for (const frame of [0, TRAY_ICON_FRAME_COUNT - 1]) {
      const name = trayIconFileName(frame, '');
      const plain = coverage(read('darwin', `${name}@2x.png`));
      const dotted = coverage(read('darwin', `${name}_notification@2x.png`));
      expect(dotted, name).toBeGreaterThan(plain * 0.6);
    }
  });
});
