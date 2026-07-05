import styled from 'styled-components';

// An open eye means "you are being watched" (exposed / connecting); a closed eye
// means "hidden in the burrow" (protected). The colour is driven by the phase so
// it matches the status title and the action button.
const Svg = styled.svg<{ $color: string; $animate: boolean }>`
  flex-shrink: 0;
  color: ${(props) => props.$color};
  transition: ${(props) => (props.$animate ? 'color 400ms ease' : 'none')};
`;

interface StatusEyeProps {
  color: string;
  closed: boolean;
  animate: boolean;
  size?: number;
}

export function StatusEye({ color, closed, animate, size = 28 }: StatusEyeProps) {
  return (
    <Svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      role="img"
      aria-hidden
      $color={color}
      $animate={animate}>
      {closed ? (
        <>
          <path d="M3 9c3 4 15 4 18 0" />
          <path d="M5 12.5 4 14.5" />
          <path d="M9 14 8.5 16.2" />
          <path d="M15 14l0.5 2.2" />
          <path d="M19 12.5 20 14.5" />
        </>
      ) : (
        <>
          <path d="M2 12c3-5.5 17-5.5 20 0-3 5.5-17 5.5-20 0Z" />
          <circle cx="12" cy="12" r="3.2" fill="currentColor" stroke="none" />
        </>
      )}
    </Svg>
  );
}
