# Repository security policy

Warren VPN is a GPL-3.0 fork of the Mullvad VPN app, and we take the security of the app
seriously. The reports in the [audits directory](audits/README.md) are Mullvad's historical
audits of the upstream app: they cover components Warren still ships (client UI, firewall and DNS
leak protection, routing) but not Warren's QUIC data plane or its wallet account model. Warren has
not yet commissioned its own external audit of the Warren-specific stack.

## Reporting security vulnerabilities

We welcome security researchers, customers or anyone else to scrutinize the source code of our
products and report any issues they find to us. We ask you to carry out responsible
research and disclosure. This includes, but is not limited to refraining from:

* Denial of service attacks against API endpoints used by the app
* Trying to disrupt the Warren VPN service
* Publicly disclosing vulnerabilities before reporting them to us in private.

Before reporting issues, we recommend that you read the following documents:
* [docs/security.md] - Explaining various expected security properties of the app
* [known issues] - Listing already known issues in the app.

**Please do not report security vulnerabilities through GitHub issues or other
public channels.** Instead please [create a vulnerability report on Github]. Or email our
support on [support@warrenbrowse.com].

[create a vulnerability report on Github]: https://github.com/WarrenBrowse/warren-app/security/advisories/new
[support@warrenbrowse.com]: mailto:support@warrenbrowse.com
[known issues]: docs/known-issues.md
[docs/security.md]: docs/security.md
