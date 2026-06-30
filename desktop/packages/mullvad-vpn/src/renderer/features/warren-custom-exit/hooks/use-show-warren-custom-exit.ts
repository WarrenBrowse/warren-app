// Gate for the advanced custom-exit setting. Dev builds only for now, so
// the escape hatch stays out of the shipped UI (mirrors `useShowDebug`).
// The daemon honours `Settings::warren_custom_exit` regardless, so a
// power user on a release build can still set it via the gRPC interface
// or `settings.json`. Flip to `true`, or wire a persisted toggle, to
// surface it on release.
export function useShowWarrenCustomExit() {
  return window.env.development;
}
