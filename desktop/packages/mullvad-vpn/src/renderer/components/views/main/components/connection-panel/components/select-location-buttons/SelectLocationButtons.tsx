import { MultiButton, SelectLocationButton, ShuffleButton } from './components';

// The location selector always carries a shuffle side button (random exit),
// matching the mockups, in every connection state.
export function SelectLocationButtons() {
  return <MultiButton mainButton={SelectLocationButton} sideButton={ShuffleButton} />;
}
