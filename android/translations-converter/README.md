# Translations converter tool

> **Do not run it on this fork.** Warren writes every Android string in each
> `values-<locale>/strings.xml` directly (see `android/README.md`), and a run rewrites those files
> from the desktop gettext catalogue alone, losing every translation that exists only on Android.
> The crate stays a workspace member until the next lockfile regeneration removes it.

A tool for helping two-way sync the translations between the Android and Desktop apps.

## How to run

Run the following command (requires `rust` and `cargo` which can be installed using [rustup.rs](https://rustup.rs/)):
```bash
cargo run
```

## Translations files

The tool creates the appropriate localized message files and directories under the
[Android project resources](android-resources) (e.g. [values-sv/strings.xml](values-sv-example))
for each locale it can find in the [`desktop/packages/mullvad-vpn/locales` directory][desktop-locales]. It will also update the
[messages.pot] template file with the string messages from the Android app for which it did not find
any translation, making it simpler to use the template for obtaining those translations.

[android-resources]: ../lib/ui/resource/src/main/res/
[desktop-locales]: ../desktop/packages/mullvad-vpn/locales/
[messages.pot]: ../desktop/packages/mullvad-vpn/locales/messages.pot
[values-sv-example]: ../lib/ui/resource/src/main/res/values-sv/strings.xml
