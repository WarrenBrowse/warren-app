# The release this flake installs.
#
# The release pipeline REGENERATES this file (ci/build-nixos-flake.sh) into the
# tarball it publishes, so what a NixOS user downloads always pins the release
# it ships with. The values committed here pin the current beta and keep the
# tree evaluable and testable in place; they are not what anybody downloads.
{
  version = "0.0.9";
  channel = "beta";
  suffix = "-beta";
  productName = "Warren VPN Beta";
  packageName = "warren-vpn-beta";
  url = "https://api.beta.warrenbrowse.com/updates/desktop/WarrenVPN-Beta-0.0.9-linux-amd64.deb";
  hash = "sha256-sA+NmHreGYxCs3MImW93Wnya2Z5Ql8b5SQmMilNm4cM=";
}
