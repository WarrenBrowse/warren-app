import React from 'react';

import { IconButtonTone } from '../icon-button';

// On the coloured header states (success/error) every foreground element
// switches to ink ('dark') for strong contrast; the neutral/default header
// keeps the light foreground.
const MainHeaderToneContext = React.createContext<IconButtonTone>('light');

export const MainHeaderToneProvider = MainHeaderToneContext.Provider;

export const useMainHeaderTone = (): IconButtonTone => React.useContext(MainHeaderToneContext);
