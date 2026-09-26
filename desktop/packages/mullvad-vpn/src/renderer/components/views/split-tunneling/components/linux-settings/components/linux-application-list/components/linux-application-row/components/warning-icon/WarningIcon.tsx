import styled from 'styled-components';

import { spacings } from '../../../../../../../../../../../lib/foundations';
import { CellTintedIcon } from '../../../../../../../../../../cell';
import { useWarningColor } from '../../hooks';

export const StyledCellWarningIcon = styled(CellTintedIcon)({
  marginInlineStart: spacings.small,
  marginInlineEnd: spacings.tiny,
});

export function WarningIcon() {
  const warningColor = useWarningColor();

  return <StyledCellWarningIcon icon="alert-circle" color={warningColor} />;
}
