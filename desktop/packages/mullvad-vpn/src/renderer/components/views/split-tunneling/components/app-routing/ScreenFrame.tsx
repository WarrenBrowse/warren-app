import React from 'react';

import { BackAction } from '../../../../keyboard-navigation';
import { StyledScreen } from './styles';

type ScreenFrameProps = {
  testId: string;
  // Where Escape, the back shortcut and the bottom button lead.
  onClose: () => void;
  children: React.ReactNode;
};

// The add, route and country screens are sheets over the list: Escape closes
// them like a dialog, on top of the app's back shortcut.
export function ScreenFrame({ testId, onClose, children }: ScreenFrameProps) {
  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) {
        event.preventDefault();
        onClose();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [onClose]);

  return (
    <BackAction action={onClose}>
      <StyledScreen data-testid={testId}>{children}</StyledScreen>
    </BackAction>
  );
}
