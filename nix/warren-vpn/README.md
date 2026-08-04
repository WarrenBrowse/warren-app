# Warren VPN on NixOS

A NixOS package and module for Warren VPN, built from the `.deb` the release
pipeline publishes. Nothing is recompiled: the store path holds the same bytes
every other Linux distribution installs, relinked against the Nix store.

The release job publishes this directory as a tarball flake beside the
installers, with `release.nix` regenerated to pin that release:

- prod: `https://api.warrenbrowse.com/updates/desktop/WarrenVPN-<version>-linux-x86_64-nixos.tar.gz`
- beta: `https://api.beta.warrenbrowse.com/updates/desktop/WarrenVPN-Beta-<version>-linux-x86_64-nixos.tar.gz`

Only `x86_64-linux` is supported: it is the one Linux artifact the pipeline
builds.

## Try it without installing anything

```
nix run https://api.beta.warrenbrowse.com/updates/desktop/WarrenVPN-Beta-<version>-linux-x86_64-nixos.tar.gz
```

That starts the desktop app. It needs a running daemon, which is what the module
below installs.

## Install it

```nix
{
  inputs.warren-vpn.url =
    "https://api.beta.warrenbrowse.com/updates/desktop/WarrenVPN-Beta-<version>-linux-x86_64-nixos.tar.gz";

  outputs = { nixpkgs, warren-vpn, ... }: {
    nixosConfigurations.my-machine = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        warren-vpn.nixosModules.default
        { services.warren-vpn-beta.enable = true; }
      ];
    };
  };
}
```

The option is named after the channel (`services.warren-vpn` on prod,
`services.warren-vpn-beta` on beta), like every installed name, so both channels
can sit on one machine without merging into a single service.

Enabling it installs the daemon, the CLI (`warren`), the desktop app, loads the
`tun` module and registers the setuid wrapper `warren-exclude` needs.

### Options

| option | default | what it does |
|---|---|---|
| `enable` | `false` | runs the daemon and installs the app |
| `package` | this flake's | the package providing the daemon, CLI and app |
| `enableExcludeWrapper` | `true` | setuid wrapper behind `warren-exclude`, which sends one process around the tunnel |
| `enableEarlyBootBlocking` | `false` | blocks traffic before the network comes up, closing the boot window |

`enableEarlyBootBlocking` is what the `.deb` installs everywhere else. It is off
by default here because it runs before any NixOS network configuration and can
fight a custom one; turn it on if you have no such configuration.

## Updating

Bump the flake input to the URL of the new release and rebuild. There is no
in-app update on NixOS: the store path is read only, so the app's update
notification is switched off and the flake is the update channel.

## Working on it in this repo

`release.nix` in the repository pins the current beta so the tree stays
evaluable and testable in place. It is not what anybody downloads: the release
job regenerates it into the published tarball.

```
nix build ./nix/warren-vpn#default
nix run ./nix/warren-vpn#default
```
