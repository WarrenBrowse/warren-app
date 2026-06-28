import { IconButton, IconButtonProps } from '../../icon-button';
import { useMainHeaderTone } from '../MainHeaderContext';

export const MainHeaderIconButton = (props: IconButtonProps) => {
  const tone = useMainHeaderTone();
  return <IconButton variant="secondary" tone={tone} {...props} />;
};
