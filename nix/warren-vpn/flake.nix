{
  description = "Warren VPN desktop client and daemon for NixOS";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

  outputs =
    { self, nixpkgs }:
    let
      release = import ./release.nix;

      # The release pipeline builds one Linux artifact, amd64. An aarch64 entry
      # here would evaluate and then fail to fetch anything.
      systems = [ "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        warren-vpn = pkgs.callPackage ./package.nix { inherit release; };
        default = warren-vpn;
      });

      overlays.default = _final: prev: {
        ${release.packageName} = prev.callPackage ./package.nix { inherit release; };
      };

      nixosModules = rec {
        warren-vpn = import ./module.nix { inherit self release; };
        default = warren-vpn;
      };

      formatter = forAllSystems (pkgs: pkgs.nixfmt-tree);
    };
}
