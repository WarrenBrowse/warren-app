# Introduction

This threat model describes the code backing Warren VPN loader and in-app updates on the two
platforms it supports (Windows and macOS). The loader is a graphical application used by Warren
users to install and upgrade the Warren VPN app on their devices, and in-app updates allows users
to update the app from within the app. The library crate `mullvad-update` (inherited crate name)
is responsible for verifying the integrity of the software that it downloads and installs on the
user's device to ensure that the software has not been tampered with. Its design allows artifacts
to be hosted on untrusted third-party hosts without compromising security; Warren currently serves
both the signed metadata and the artifacts from its own update host
(`api.warrenbrowse.com/updates`).

These tools perform network requests towards the Warren API host and requires both read & write
access to the target device file system.

## Acquiring Warren VPN loader

The loader application is initially downloaded from Warren's website
([warrenbrowse.com](https://warrenbrowse.com)) or the Warren VPN app GitHub repository
([github.com/WarrenBrowse/warren-app](https://github.com/WarrenBrowse/warren-app)). The update
metadata consumed by `mullvad-update` is signed with Warren's release signing key, and release
artifacts are checksummed in that signed metadata.

# Who do we trust

Some Warren maintainers - Access to publish metadata information to be consumed by
`mullvad-update` is segmented and has been granted to select individuals which are trusted to make
app releases. The release signing key is held only in a dedicated, secured signing environment.

# Who is the attacker

## Nation states and law enforcement

With the goal of de-anonymizing individuals in order to track them and disarm “dissidents”.

## Crooks

With the goal to …

* Install malware on target devices

* Make our users part of botnets

* Steal users' information (crypto wallets etc)

# Capabilities of the attacker

* Changing what is served from the update host or any mirror in front of it

  * Serving malicious software or version metadata
  * Serving legitimate, but old versions of the version metadata or app binaries with known
    vulnerabilities
  * Serving files large enough to fill up the targets disk/ram

* Modify the downloaded installer on the client machine, tricking the `mullvad-update`
  mechanism to run a malicious installer with admin privileges. The result is that
  the attacker can escalate their foothold on the client machine from regular
  user to administrator.

# Countermeasures

Here are countermeasures we have identified against the above attackers which have been implemented
in `mullvad-update` and the loader/in-app upgrade mechanisms:

* Attach a signature to the metadata, and verify it on the client before using it

* Attach an expiry date to the signed part of the metadata, and don't use any expired metadata

* Attach an always increasing counter to the signed part of the metadata, and don't
  use any metadata with a lower counter than the highest previously observed valid counter

* Attach checksums of installer artifacts in the metadata, and verify that all downloaded artifacts
  has this expected checksum

* Attach the size of installer artifacts in the metadata, and abort any download if more than the
  expected amount of data is returned.

* Abort downloading the metadata if it is larger than a hardcoded max size

* Only allow trusted people to publish metadata, from a dedicated, secured signing environment

* When relevant, only read/use downloaded software artifacts from a location that the loader (or
  admin) controls, to prevent privilege escalation


# Out of scope

* Most attacks involving physical access to the user's computer are not protected against.

* Malicious code that runs on the user's computer should not be able to use this software
  to escalate to higher privileges. But other than that, this threat model does
  not consider such an attacker.
