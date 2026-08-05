import { useCallback } from 'react';
import styled from 'styled-components';

import { ChangelogBlock, ChangelogInline, IChangelog } from '../../../shared/ipc-types';
import { BodySmall, BodySmallSemiBold, Flex } from '../../lib/components';
import { Link } from '../../lib/components/link';
import { colors, spacings } from '../../lib/foundations';

const StyledList = styled.ul`
  display: flex;
  flex-direction: column;
  gap: ${spacings.medium};
  list-style-type: disc;
  padding-left: 0;
  li {
    margin-left: 1.5em;
  }
`;

const StyledCode = styled.code`
  font-family: monospace;
  color: ${colors.white};
`;

export type ChangelogListProps = {
  changelog: IChangelog;
};

export function ChangelogList({ changelog }: ChangelogListProps) {
  return (
    <Flex flexDirection="column" gap="medium">
      {changelog.map((block, index) => (
        <ChangelogBlockView key={index} block={block} />
      ))}
    </Flex>
  );
}

function ChangelogBlockView({ block }: { block: ChangelogBlock }) {
  switch (block.type) {
    case 'heading':
      // The notes only ever nest two levels deep ("Added", then a component
      // name), and the view already carries the version as its own heading, so
      // every level renders the same: one emphasised line above its entries.
      return (
        <BodySmallSemiBold as="h3" color="white">
          <InlineRuns content={block.content} />
        </BodySmallSemiBold>
      );
    case 'paragraph':
      return (
        <BodySmall color="whiteAlpha60">
          <InlineRuns content={block.content} />
        </BodySmall>
      );
    case 'list':
      return (
        <StyledList>
          {block.items.map((item, index) => (
            <BodySmall as="li" key={index} color="whiteAlpha60">
              <InlineRuns content={item} />
            </BodySmall>
          ))}
        </StyledList>
      );
  }
}

function InlineRuns({ content }: { content: ChangelogInline[] }) {
  return (
    <>
      {content.map((run, index) => {
        switch (run.type) {
          case 'strong':
            return <strong key={index}>{run.value}</strong>;
          case 'code':
            return <StyledCode key={index}>{run.value}</StyledCode>;
          case 'link':
            return <ChangelogLink key={index} href={run.href} label={run.value} />;
          case 'text':
            return <span key={index}>{run.value}</span>;
        }
      })}
    </>
  );
}

function ChangelogLink({ href, label }: { href: string; label: string }) {
  // Straight to the IPC channel rather than the app context's `openUrl`, whose
  // `Url` type deliberately enumerates the app's own static links. A changelog
  // target is not one of those: it arrives inside the signature-verified update
  // manifest, and the parser already refused anything that is not http(s).
  const navigate = useCallback(
    (event: React.MouseEvent<HTMLAnchorElement>) => {
      event.preventDefault();
      return window.ipc.app.openUrl(href);
    },
    [href],
  );

  return (
    <Link href="" onClick={navigate}>
      {label}
    </Link>
  );
}
