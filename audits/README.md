# Audits, pentests and external security reviews

> **Scope note for Warren.** The reports listed below are **Mullvad VPN's** historical audits,
> inherited with this fork. They cover the upstream Mullvad app and its **WireGuard** data plane.
> Warren replaces that data plane with a custom **QUIC tunnel** (`talpid-warren-tunnel` +
> `warren-core`) and adds a wallet-based account model, **none of which is covered by these
> reports**. They are kept for the audited components Warren still ships (client UI, firewall/DNS
> leak protection, routing) and for transparency, not as an audit of the Warren-specific stack.

Independent audits help to discover potential security vulnerabilities and fix them, all resulting
in an even better service. It also gives you the opportunity to judge whether or not we are
technically competent enough to provide a service in which security is paramount.

These external security audits were performed on the upstream Mullvad app. Here are all the audits
performed so far:

* [2018-09-24 - Assured and Cure53](./2018-09-24-assured-cure53.md)
* [2020-06-12 - Cure53](./2020-06-12-cure53.md)
* [2022-10-14 - Atredis](./2022-10-14-atredis.md)
* [2024-12-10 - X41 D-Sec](./2024-12-10-X41-D-Sec.md)

## Additional audits and certifications

Apart from the biannual audits mentioned above, we've also conducted the following:

* [2025-02-24 - NCC Group Mobile Application Security Assessment (MASA) of the Android app](./2025-02-24-nccgroup-android-masa.md)
* [2025-03-20 - Audit of the installer downloader by Assured](./2025-03-20-assured-installer-downloader.md)