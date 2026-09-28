import styled from 'styled-components';

import { colors, spacings } from '../../../../../lib/foundations';
import { sourceSansPro } from '../../../../common-styles';

// Shared pieces of the App routing screens.

export const StyledSection = styled.section({
  display: 'flex',
  flexDirection: 'column',
  gap: '10px',
  margin: `0 ${spacings.medium} 20px`,
});

export const StyledSectionTitle = styled.h2({
  ...sourceSansPro,
  margin: 0,
  fontSize: '17px',
  lineHeight: '22px',
  fontWeight: 700,
  color: colors.white,
});

export const StyledHelp = styled.p({
  ...sourceSansPro,
  margin: 0,
  fontWeight: 400,
  fontSize: '14px',
  lineHeight: '20px',
  color: colors.whiteAlpha60,
});

export const StyledCard = styled.div({
  ...sourceSansPro,
  borderRadius: '10px',
  padding: '14px 16px',
  backgroundColor: colors.blue10,
  fontSize: '15px',
  fontWeight: 400,
  lineHeight: '22px',
  color: colors.whiteAlpha60,
});

// The ocre of the brand on the background, for the one warning of the view.
export const StyledWarningCard = styled(StyledCard)({
  display: 'flex',
  gap: '10px',
  alignItems: 'flex-start',
  backgroundColor: 'rgba(202, 150, 60, 0.15)',
  color: colors.nose,
  '& > svg': {
    marginTop: '2px',
  },
});

// The pill on a rule and on the "+ App" button.
export const pill = {
  ...sourceSansPro,
  display: 'inline-flex',
  alignItems: 'center',
  gap: '8px',
  minHeight: '34px',
  borderRadius: '999px',
  border: `1.5px solid ${colors.whiteOnBlue20}`,
  fontSize: '15px',
  fontWeight: 400,
  whiteSpace: 'nowrap',
  color: colors.white,
} as const;

// A full-height screen of the flow: a title, a body that scrolls, and the
// buttons at the bottom.
export const StyledScreen = styled.div({
  ...sourceSansPro,
  flex: 1,
  minHeight: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: spacings.medium,
  padding: `${spacings.large} ${spacings.medium} ${spacings.medium}`,
  backgroundColor: colors.darkBlue,
  color: colors.white,
});

export const StyledScreenTitle = styled.h1({
  ...sourceSansPro,
  margin: 0,
  fontSize: '30px',
  lineHeight: '34px',
  fontWeight: 700,
  color: colors.white,
  overflowWrap: 'anywhere',
});

export const StyledScreenBody = styled.div({
  flex: 1,
  minHeight: 0,
  overflowY: 'auto',
  display: 'flex',
  flexDirection: 'column',
  gap: spacings.small,
});

export const StyledScreenButton = styled.button({
  ...sourceSansPro,
  flexShrink: 0,
  minHeight: '48px',
  border: 'none',
  borderRadius: '8px',
  backgroundColor: colors.blue,
  color: colors.white,
  fontSize: '18px',
  fontWeight: 400,
  textAlign: 'center',
  justifyContent: 'center',
  cursor: 'default',
  '&&:hover': {
    backgroundColor: colors.blue60,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '2px',
  },
});

export const StyledTextButton = styled.button({
  ...sourceSansPro,
  flexShrink: 0,
  minHeight: '44px',
  border: 'none',
  borderRadius: '8px',
  background: 'transparent',
  color: colors.redText,
  fontSize: '17px',
  fontWeight: 400,
  textAlign: 'center',
  cursor: 'default',
  '&&:hover': {
    backgroundColor: colors.blue10,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '2px',
  },
});

// A row of a list: an app, a country, an option.
export const StyledRowButton = styled.button<{ $selected?: boolean }>((props) => ({
  ...sourceSansPro,
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  width: '100%',
  flexShrink: 0,
  minHeight: '56px',
  padding: '8px 14px',
  border: `2px solid ${props.$selected ? colors.green : colors.blue40}`,
  borderRadius: '10px',
  backgroundColor: colors.blue40,
  color: colors.white,
  fontWeight: 400,
  textAlign: 'start',
  cursor: 'default',
  '&&:not(:disabled):hover': {
    backgroundColor: colors.blue50,
    borderColor: props.$selected ? colors.green : colors.blue50,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '1px',
  },
  '&&:disabled': {
    opacity: 0.5,
  },
}));

export const StyledRowName = styled.span({
  flex: 1,
  minWidth: 0,
  fontSize: '18px',
  lineHeight: '24px',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
});
