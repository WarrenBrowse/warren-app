import { useEffect, useRef, useState } from 'react';
import styled from 'styled-components';

import { useMeasure } from '../../hooks/useMeasure';
import { getConnectionPhase } from '../../lib/connection-phase';
import { useExitEgressDead } from '../../lib/exit-egress';
import { homeClearTop, homeTokens, surfaces } from '../../lib/foundations';
import { getReduceMotion } from '../../lib/functions';
import { useHostOffline } from '../../lib/host-offline';
import { useSelector } from '../../redux/store';
import {
  CANVAS_HEIGHT,
  CANVAS_WIDTH,
  COMPACT_TRANSITION_MS,
  FEATHER_ROWS,
  Frame,
  frameScenery,
  framingSpan,
  HERO_COMPACT_RATIO,
  HERO_HEIGHT_RATIO,
  HERO_MASK_STOPS,
  heroFade,
  phaseLandscape,
} from './framing';
import { BULA_IMAGE, resolveScenery, TERRIER_IMAGE } from './scenery';

const { hero } = homeTokens;

// Fades the hero into the page along the mockup's curve: the panel's status
// block rides up over the last rows, so the art has no edge.
const fadeMask = `linear-gradient(to bottom, ${HERO_MASK_STOPS.map(
  ([at, alpha]) => `rgba(0, 0, 0, ${alpha}) ${at * 100}%`,
).join(', ')})`;

const Root = styled.div<{ $fill: string; $animate: boolean }>`
  position: absolute;
  inset: 0;
  overflow: hidden;
  background-color: ${(props) => props.$fill};
  transition: ${(props) =>
    props.$animate ? `background-color ${hero.crossfadeMs}ms ease` : 'none'};
  -webkit-mask-image: ${fadeMask};
  mask-image: ${fadeMask};
`;

// The registered canvas: the three layers fill it exactly, so moving or
// scaling it moves them together. Edges facing a headroom strip or a side
// margin are feathered into the top fill behind it.
// A reframe that follows the details opening keeps the hero's own pace, so
// the art and the hero edge move as one.
const reframe = (ms: number, easing: string) =>
  ['top', 'left', 'width', 'height'].map((property) => `${property} ${ms}ms ${easing}`).join(', ');

const Canvas = styled.div<{ $animate: boolean; $resize: boolean }>`
  position: absolute;
  transition: ${(props) =>
    !props.$animate
      ? 'none'
      : props.$resize
        ? reframe(COMPACT_TRANSITION_MS, 'cubic-bezier(0.3, 0.7, 0.2, 1)')
        : reframe(hero.frameMs, 'ease-in-out')};
`;

const Layer = styled.img`
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  user-select: none;
  -webkit-user-drag: none;
`;

// Blur and a slight zoom while the tunnel is not settled; both landscapes of a
// cross-fade live inside so they blur together.
const Scene = styled.div<{ $blurred: boolean; $animate: boolean }>`
  position: absolute;
  inset: 0;
  filter: ${(props) =>
    props.$blurred ? `blur(${hero.blurRadius}px) brightness(0.92)` : 'blur(0) brightness(1)'};
  transform: ${(props) => (props.$blurred ? 'scale(1.08)' : 'scale(1)')};
  transition: ${(props) =>
    props.$animate ? 'filter 900ms ease, transform 6000ms ease-out' : 'none'};
  will-change: filter, transform;
`;

const FrontLandscape = styled(Layer)<{ $animate: boolean }>`
  animation: ${(props) =>
    props.$animate ? `warren-scenery-fade ${hero.crossfadeMs}ms ease forwards` : 'none'};
  @keyframes warren-scenery-fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
`;

// Bula sits on the meadow while exposed and slips into the burrow, sliding down
// and fading out, once the tunnel is up.
const Bula = styled(Layer)<{ $visible: boolean; $animate: boolean }>`
  opacity: ${(props) => (props.$visible ? 1 : 0)};
  transform: translateY(${(props) => (props.$visible ? 0 : 3)}%);
  transition: ${(props) =>
    props.$animate ? `opacity ${hero.bulaMs}ms ease, transform ${hero.bulaMs}ms ease` : 'none'};
`;

// The page colour over the top of the sky, so the toolbar reads on the
// brightest plates, and a paper glow behind the inked logo.
const Scrim = styled.div`
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: linear-gradient(
    to bottom,
    ${surfaces.homeScrim} 0%,
    transparent ${hero.scrimStop * 100}%
  );
`;

const LogoGlow = styled.div`
  position: absolute;
  left: 0;
  top: 0;
  width: ${hero.glowWidth}px;
  height: ${hero.glowHeight}px;
  pointer-events: none;
  background: radial-gradient(
    ellipse 100% 100% at 0% 0%,
    ${surfaces.homeLogoGlow} 0%,
    color-mix(in srgb, ${surfaces.homeLogoGlow} 43%, transparent) 45%,
    transparent 75%
  );
`;

function featherMask(frame: Frame): string | undefined {
  const masks: string[] = [];
  const feather = (FEATHER_ROWS / CANVAS_HEIGHT) * 100;
  if (frame.headroom > 0) {
    masks.push(`linear-gradient(to bottom, transparent 0%, #000 ${feather}%)`);
  }
  if (frame.left > 0) {
    const side = (FEATHER_ROWS / CANVAS_WIDTH) * 100;
    masks.push(
      `linear-gradient(to right, transparent 0%, #000 ${side}%, #000 ${100 - side}%, transparent 100%)`,
    );
  }
  return masks.length > 0 ? masks.join(', ') : undefined;
}

// The country the app is connecting to or connected to. Prefers the live geoip
// exit country; while it resolves during connecting, falls back to the selected
// relay constraint so the target city shows from the first frame.
export function useTargetCountry(): string | undefined {
  const status = useSelector((state) => state.connection.status);
  const relaySettings = useSelector((state) => state.settings.relaySettings);
  const relayLocations = useSelector((state) => state.settings.relayLocations);

  const liveCountry =
    status.state === 'connected' || status.state === 'connecting'
      ? status.details?.location?.country
      : undefined;
  if (liveCountry) {
    return liveCountry;
  }

  if (status.state !== 'connecting') {
    return undefined;
  }
  if ('normal' in relaySettings) {
    const location = relaySettings.normal.location;
    if (location !== 'any' && 'country' in location && location.country) {
      // relayLocations names are the English relay-list country names, which is
      // exactly what the scenery lookup keys on.
      return relayLocations.find((country) => country.code === location.country)?.name;
    }
  }
  return undefined;
}

// The home hero: the scenery framed so that what matters in the current phase
// stays clear of the floating header and of the fade into the panel. While
// `compact` (the details are open) the hero shrinks and the art keeps its
// width, giving its lower part to the fade.
export default function CountryBackdrop({ compact = false }: { compact?: boolean }) {
  const status = useSelector((state) => state.connection.status);
  const hostOffline = useHostOffline();
  const exitEgressDead = useExitEgressDead();
  const targetCountry = useTargetCountry();
  const [ref, size] = useMeasure<HTMLDivElement>();
  const resizing = useResizing(compact);

  // e2e runs against deterministic snapshots; skip the decorative scene.
  if (window.env.e2e) {
    return null;
  }

  const phase = getConnectionPhase(status, hostOffline, exitEgressDead);
  const scenery = resolveScenery(phase, targetCountry);
  const landscape = phaseLandscape(phase, targetCountry);
  const animate = !getReduceMotion();
  // Framed for the height the hero is heading to, not the one it is passing
  // through, so the art never shrinks to fit a hero halfway open.
  const height = size.width * (compact ? HERO_COMPACT_RATIO : HERO_HEIGHT_RATIO);
  const frame =
    size.width > 0
      ? frameScenery({
          width: size.width,
          height,
          clearTop: homeClearTop,
          fade: heroFade(height),
          span: framingSpan(phase, targetCountry),
          compact,
          fullHeight: size.width * HERO_HEIGHT_RATIO,
        })
      : undefined;
  const mask = frame ? featherMask(frame) : undefined;

  return (
    <Root ref={ref} $fill={landscape.topFill} $animate={animate} aria-hidden>
      {frame && (
        <Canvas
          $animate={animate}
          $resize={resizing}
          data-testid="scenery-canvas"
          style={{
            left: frame.left,
            top: frame.top,
            width: frame.width,
            height: frame.height,
            WebkitMaskImage: mask,
            maskImage: mask,
            WebkitMaskComposite: mask?.includes('to right') ? 'source-in' : undefined,
            maskComposite: mask?.includes('to right') ? 'intersect' : undefined,
          }}>
          <CrossfadeLandscape src={scenery.image} animate={animate} blurred={scenery.blurred} />
          <Layer src={TERRIER_IMAGE} alt="" />
          <Bula src={BULA_IMAGE} alt="" $visible={scenery.showBula} $animate={animate} />
        </Canvas>
      )}
      <Scrim />
      <LogoGlow />
    </Root>
  );
}

// True for the length of the hero's own resize after `compact` flips.
function useResizing(compact: boolean): boolean {
  const [resizing, setResizing] = useState(false);
  const previous = useRef(compact);
  useEffect(() => {
    if (previous.current === compact) return;
    previous.current = compact;
    setResizing(true);
    const timer = setTimeout(() => setResizing(false), COMPACT_TRANSITION_MS);
    return () => clearTimeout(timer);
  }, [compact]);
  return resizing || previous.current !== compact;
}

interface CrossfadeLandscapeProps {
  src: string;
  animate: boolean;
  blurred: boolean;
}

// Keeps at most two landscapes: the previous one (opaque, back) and the current
// one (fading in, front). Nothing to prune because the front eventually fully
// covers the back.
function CrossfadeLandscape({ src, animate, blurred }: CrossfadeLandscapeProps) {
  const [layers, setLayers] = useState({ front: src, back: src, key: 0 });
  const prevSrc = useRef(src);

  useEffect(() => {
    if (prevSrc.current !== src) {
      setLayers((current) => ({ front: src, back: current.front, key: current.key + 1 }));
      prevSrc.current = src;
    }
  }, [src]);

  return (
    <Scene $blurred={blurred} $animate={animate}>
      <Layer src={layers.back} alt="" />
      <FrontLandscape key={layers.key} src={layers.front} alt="" $animate={animate} />
    </Scene>
  );
}
