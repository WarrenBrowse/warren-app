import { useEffect, useRef, useState } from 'react';
import styled from 'styled-components';

import { getConnectionPhase, getPhaseAccentColor } from '../../lib/connection-phase';
import { getReduceMotion } from '../../lib/functions';
import { useHostOffline } from '../../lib/host-offline';
import { useSelector } from '../../redux/store';
import { BULA_IMAGE, resolveScenery, TERRIER_IMAGE } from './scenery';

// The whole scene sits behind the connection panel (which owns z-index 1).
const Root = styled.div`
  position: absolute;
  inset: 0;
  overflow: hidden;
  background-color: rgb(120, 170, 210); /* sky tone shown before art paints */
`;

// Wrapper carrying the blur+scale of the connecting animation. Both landscape
// layers live inside so they blur together during a cross-fade.
const Scene = styled.div<{ $blurred: boolean; $animate: boolean }>`
  position: absolute;
  inset: 0;
  filter: ${(props) => (props.$blurred ? 'blur(14px) brightness(0.92)' : 'blur(0) brightness(1)')};
  transform: ${(props) => (props.$blurred ? 'scale(1.08)' : 'scale(1)')};
  transition: ${(props) =>
    props.$animate ? 'filter 900ms ease, transform 6000ms ease-out' : 'none'};
  will-change: filter, transform;
`;

const FullBleed = styled.img`
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: center;
  user-select: none;
  -webkit-user-drag: none;
`;

// The incoming landscape fades in over the previous one (which stays opaque
// underneath), producing a cross-fade without a background-image transition.
const FrontLandscape = styled(FullBleed)<{ $animate: boolean }>`
  animation: ${(props) => (props.$animate ? 'warren-scenery-fade 700ms ease forwards' : 'none')};
  @keyframes warren-scenery-fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
`;

// The foreground (burrow + Bula) is raised by a constant share of the frame
// height so the burrow mouth and the rabbit clear the panel. It is the SAME in
// every phase on purpose: only the background crossfades between modes, so the
// foreground never jumps. The seam it uncovers at the bottom stays behind the
// panel. Kept small so the burrow stays discreet over a cityscape.
const LIFT_PERCENT = 8;

// The burrow foreground, always at the raised framing position.
const Foreground = styled(FullBleed)`
  transform: translateY(-${LIFT_PERCENT}%);
`;

// Bula sits exposed outside the burrow; when protected he is hidden inside it,
// sliding down and fading out. He rides the same lift as the burrow he sits on.
const Bula = styled(FullBleed)<{ $visible: boolean; $animate: boolean }>`
  opacity: ${(props) => (props.$visible ? 1 : 0)};
  transform: translateY(${(props) => -LIFT_PERCENT + (props.$visible ? 0 : 3)}%);
  transition: ${(props) => (props.$animate ? 'opacity 550ms ease, transform 550ms ease' : 'none')};
`;

// A faint accent tint reinforces the phase (red exposed / orange connecting /
// green protected) without washing out the artwork.
const AccentWash = styled.div<{ $color: string; $animate: boolean }>`
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: linear-gradient(
    to bottom,
    ${(props) => props.$color} 0%,
    transparent 22%,
    transparent 78%,
    ${(props) => props.$color} 100%
  );
  opacity: 0.14;
  mix-blend-mode: soft-light;
  transition: ${(props) => (props.$animate ? 'background 700ms ease' : 'none')};
`;

// Country the app is connecting to / connected to. Prefer the live geoip exit
// country; while it resolves during connecting, fall back to the selected relay
// constraint so the target city shows from the first frame instead of a blurred
// generic plain.
function useTargetCountry(): string | undefined {
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

export default function CountryBackdrop() {
  const status = useSelector((state) => state.connection.status);
  const hostOffline = useHostOffline();
  const targetCountry = useTargetCountry();

  // e2e runs against deterministic snapshots; skip the decorative scene.
  if (window.env.e2e) {
    return null;
  }

  const phase = getConnectionPhase(status, hostOffline);
  const scenery = resolveScenery(phase, targetCountry);
  const animate = !getReduceMotion();
  const accent = getPhaseAccentColor(phase);

  return (
    <Root>
      <CrossfadeLandscape src={scenery.image} animate={animate} blurred={scenery.blurred} />
      <Foreground src={TERRIER_IMAGE} alt="" aria-hidden />
      <Bula src={BULA_IMAGE} alt="" aria-hidden $visible={scenery.showBula} $animate={animate} />
      <AccentWash $color={accent} $animate={animate} />
    </Root>
  );
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
      <FullBleed src={layers.back} alt="" aria-hidden />
      <FrontLandscape key={layers.key} src={layers.front} alt="" aria-hidden $animate={animate} />
    </Scene>
  );
}
