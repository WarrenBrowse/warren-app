// Warren fork — Phase H : feature des toggles persistants Warren.
//
// L'écran Settings consomme ces composants pour exposer les deux
// toggles à l'utilisateur. Le restart du daemon est requis pour
// appliquer un changement (cf. `warren_mode::resolve` côté Rust).

export {
  WarrenModeSwitch,
  WarrenLocalAccountSwitch,
  WarrenModeSetting,
  WarrenLocalAccountSetting,
  WarrenApiUrlSetting,
} from './components';
export { useWarrenMode, useWarrenLocalAccount, useWarrenApiUrl } from './hooks';
