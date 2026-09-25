import styled from 'styled-components';

import * as Cell from '../../../../components/cell';
import { colors } from '../../../../lib/foundations';

export const StyledNetworkContainer = styled(Cell.Container)({
  display: 'flex',
  backgroundColor: colors.blue40,
});

export const StyledActionContainer = styled(Cell.Container)({
  display: 'flex',
  backgroundColor: colors.blue20,
  '&&:hover': {
    backgroundColor: colors.blue60,
  },
});

export const StyledActionButton = styled.button({
  display: 'flex',
  alignItems: 'center',
  flex: 1,
  border: 'none',
  background: colors.transparent,
  padding: 0,
  margin: 0,
});

export const StyledLabel = styled(Cell.Label)({
  fontFamily: 'Open Sans',
  fontWeight: 400,
  fontSize: '16px',
  paddingLeft: '32px',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'break-word',
  marginRight: '25px',
});
