# How to contribute to the Warren VPN app

The Warren VPN app is open source so that
* users can verify that the app does what we say it does, and build it from source instead of
  trusting our released binaries
* third parties can contribute to it.


## Filing issues

If you find a bug in the app's code, report it in the
[GitHub issue tracker](https://github.com/WarrenBrowse/warren-app/issues). Send every other problem
or question (connection issues, your subscription, the servers) to
[support@warrenbrowse.com](mailto:support@warrenbrowse.com) or to the
[Warren forum](https://forum.warrenbrowse.com).

Security issues go through the private channel described in [SECURITY.md](SECURITY.md), never
through a public issue.


## Submitting changes

Read the sections below, then open a pull request on GitHub. Which changes are merged is decided by
the maintainers, based on the development plans for the app. Before a new feature, a refactoring
or any large change, describe it on the [Warren forum](https://forum.warrenbrowse.com) or in an
issue first. For a bug fix, comment on the existing issue or open a
[bug report](https://github.com/WarrenBrowse/warren-app/issues/new/choose).

### How changes are written here

Most changes to this repository are written by AI coding agents working under the rules in
[CLAUDE.md](CLAUDE.md), and pull requests are reviewed automatically before a merge. An outside
contribution is welcome whether or not an AI helped write it, provided a person has read all of it
and answers the questions the review raises.

### Rules every change follows

* Test first: write a test that describes the behavior and fails, then the code that makes it
  pass. A test must be able to fail when the code it covers is broken.
* Code, comments, identifiers and commit messages are in English.
* Commits follow [Conventional Commits](https://www.conventionalcommits.org/), with a subject line
  and no body.
* No em-dash or en-dash anywhere, translations included: use a comma, a colon, parentheses or a
  period.
* No public key, wallet address, IP address, nonce or seed in a log line or an error message.
* The wire formats shared with the server are pinned by the golden vectors in `vectors/`. Fix the
  code, never the vector.

### Localization / translations

Warren translations are maintained in this repository. To improve a translation, open a pull
request against the PO file, the Android `strings.xml` or the iOS `.xcstrings` catalog of that
language, or report it on the [Warren forum](https://forum.warrenbrowse.com).

### Copyright and licensing of contributions

This repository is licensed under the GNU General Public License version 3 or later
(`GPL-3.0-or-later`), see [LICENSE.md](LICENSE.md). Anything you submit to it (code, binaries,
tools, documentation, graphics or any other material) is licensed under the same terms. Submitting
means that you are the author of the whole contribution, or have the right to submit it under
those terms. No contributor agreement grants anyone further rights over your contribution.

The tunnel engine and the client SDK this app is built with live in their own repositories
(`warrenguard`, `warren-sdk-rs`, `warren-contract`) under the GNU Affero General Public License
version 3 or later; contributions to them follow their own `CONTRIBUTING` and licence files.
