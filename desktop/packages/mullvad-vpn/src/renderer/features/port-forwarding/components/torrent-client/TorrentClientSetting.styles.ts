import styled from 'styled-components';

/** One label and its control, label left and control right, the same shape as
 * a rule row in the port editor above. */
export const StyledField = styled.div({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  width: '100%',
  gap: '12px',
});

export const StyledInput = styled.input({
  background: 'transparent',
  border: 'none',
  borderBottom: '1px solid rgba(255,255,255,0.4)',
  color: 'white',
  fontFamily: 'inherit',
  fontSize: '14px',
  padding: '4px 0',
  textAlign: 'end',
  minWidth: 0,
  flex: '1 1 20ch',
  '&:focus': {
    outline: 'none',
    borderBottomColor: 'white',
  },
});

export const StyledSelect = styled.select({
  background: 'transparent',
  border: 'none',
  borderBottom: '1px solid rgba(255,255,255,0.4)',
  color: 'white',
  fontFamily: 'inherit',
  fontSize: '14px',
  padding: '4px 0',
  cursor: 'pointer',
  '&:focus': {
    outline: 'none',
    borderBottomColor: 'white',
  },
  '& option': {
    color: 'black',
  },
});

export const StyledActions = styled.div({
  display: 'flex',
  flexWrap: 'wrap',
  gap: '16px',
  paddingTop: '4px',
});

export const StyledButton = styled.button<{ $disabled: boolean }>(({ $disabled }) => ({
  background: 'transparent',
  border: 'none',
  color: $disabled ? 'rgba(255,255,255,0.3)' : '#44ad4d',
  cursor: $disabled ? 'not-allowed' : 'pointer',
  fontFamily: 'inherit',
  fontSize: '13px',
  padding: 0,
  textAlign: 'start',
  '&:hover': {
    textDecoration: $disabled ? 'none' : 'underline',
  },
}));
