import React from 'react';

import { IconButtonTone } from '../icon-button';

const MainHeaderToneContext = React.createContext<IconButtonTone>('light');

export const MainHeaderToneProvider = MainHeaderToneContext.Provider;

export const useMainHeaderTone = (): IconButtonTone => React.useContext(MainHeaderToneContext);
