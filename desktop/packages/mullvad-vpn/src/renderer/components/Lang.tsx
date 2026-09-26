import { PropsWithChildren } from 'react';
import styled from 'styled-components';

const StyledLang = styled.div({
  display: 'flex',
  flex: '1',
  maxWidth: '100%',
});

// The language and the text direction are set on the document root (see
// `applyDocumentLocale`), which also covers what renders in portals.
export default function Lang(props: PropsWithChildren) {
  return <StyledLang>{props.children}</StyledLang>;
}
