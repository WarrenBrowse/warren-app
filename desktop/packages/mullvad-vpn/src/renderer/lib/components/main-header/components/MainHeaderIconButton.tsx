import { IconButton, IconButtonProps } from '../../icon-button';
import { useMainHeaderTone } from '../MainHeaderContext';

export const MainHeaderIconButton = (props: IconButtonProps) => {
  const tone = useMainHeaderTone();
  // Primary (solid) rather than the softer secondary: over the bright scenery the
  // account/settings icons need to read as crisp flat black, per the mockups.
  return <IconButton variant="primary" tone={tone} {...props} />;
};
