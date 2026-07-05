import { useEffect, useRef, useState } from 'react';
import styled from 'styled-components';

import { getConnectionPhase, getPhaseAccentColor } from '../../lib/connection-phase';
import { getReduceMotion } from '../../lib/functions';
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

// Bula sits exposed outside the burrow; when protected he is hidden inside it,
// sliding down and fading out.
const Bula = styled(FullBleed)<{ $visible: boolean; $animate: boolean }>`
  opacity: ${(props) => (props.$visible ? 1 : 0)};
  transform: ${(props) => (props.$visible ? 'translateY(0)' : 'translateY(3%)')};
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

export default function CountryBackdrop() {
  const status = useSelector((state) => state.connection.status);

  // e2e runs against deterministic snapshots; skip the decorative scene.
  if (window.env.e2e) {
    return null;
  }

  const phase = getConnectionPhase(status.state);
  const exitCountry =
    status.state === 'connected' || status.state === 'connecting'
      ? status.details?.location?.country
      : undefined;

  const scenery = resolveScenery(phase, exitCountry);
  const animate = !getReduceMotion();
  const accent = getPhaseAccentColor(phase);

  return (
    <Root>
      <CrossfadeLandscape src={scenery.image} animate={animate} blurred={scenery.blurred} />
      <FullBleed src={TERRIER_IMAGE} alt="" aria-hidden />
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
  const [frontSrc, setFrontSrc] = useState(src);
  const [backSrc, setBackSrc] = useState(src);
  const fadeKey = useRef(0);

  useEffect(() => {
    setFrontSrc((prevFront) => {
      if (prevFront !== src) {
        setBackSrc(prevFront);
        fadeKey.current += 1;
      }
      return src;
    });
  }, [src]);

  return (
    <Scene $blurred={blurred} $animate={animate}>
      <FullBleed src={backSrc} alt="" aria-hidden />
      <FrontLandscape key={fadeKey.current} src={frontSrc} alt="" aria-hidden $animate={animate} />
    </Scene>
  );
}
