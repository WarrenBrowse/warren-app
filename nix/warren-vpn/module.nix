# NixOS integration for Warren VPN. The .deb's maintainer scripts have no
# equivalent here: enabling the units, the setuid helper and the tun module is
# what this module does declaratively, so `nixos-rebuild switch` is the whole
# install.
{ self, release }:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  defaultPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.default;

  inherit (lib)
    getExe'
    mkEnableOption
    mkIf
    mkOption
    optional
    types
    ;

  # The option path carries the channel, exactly like the installed names do, so
  # a machine can hold the prod and the beta module at once without the two
  # silently merging into one service.
  optionName = release.packageName;
  cfg = config.services.${optionName};

  daemon = "warren-daemon${release.suffix}";
  earlyBoot = "warren-early-boot-blocking${release.suffix}";
in
{
  options.services.${optionName} = {
    enable = mkEnableOption "the Warren VPN daemon";

    package = mkOption {
      type = types.package;
      default = defaultPackage;
      defaultText = lib.literalExpression "warren-vpn.packages.\${system}.default";
      description = "The Warren VPN package providing the daemon, the CLI and the desktop app.";
    };

    enableExcludeWrapper =
      mkEnableOption ''
        the setuid wrapper behind `warren-exclude`, the command that sends a single
        process straight to the clearnet around the tunnel. Turn it off on a machine
        where setuid binaries are a concern: everything else keeps working
      ''
      // {
        default = true;
      };

    enableEarlyBootBlocking = mkEnableOption ''
      a unit that blocks all traffic before the network is configured, closing the
      window between boot and the daemon taking over. It matches what the .deb
      installs on other distributions, and is off by default here because it runs
      before any NixOS network configuration and can fight a custom one
    '';
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.package.hasWarrenDaemon or false;
        message = ''
          services.${optionName}.package reports no Warren VPN daemon.
          Leave it unset to use the package this flake provides.
        '';
      }
    ];

    boot.kernelModules = [ "tun" ];

    environment.systemPackages = [ cfg.package ];

    # The daemon drops a single process out of the tunnel by moving it into a
    # net_cls cgroup, which needs privileges the calling user does not have.
    security.wrappers.${"warren-exclude${release.suffix}"} = mkIf cfg.enableExcludeWrapper {
      setuid = true;
      owner = "root";
      group = "root";
      source = getExe' cfg.package "warren-exclude${release.suffix}";
    };

    # The daemon writes the tunnel's DNS through systemd-resolved when it is
    # there, and falls back to rewriting /etc/resolv.conf when it is not.
    services.resolved.enable = lib.mkDefault true;

    # So the desktop's own network indicator shows a VPN is up. Both halves
    # are needed: NetworkManager only loads plugins from packages listed
    # here, and the system bus denies a name nothing grants, so without the
    # D-Bus policy the plugin cannot take the name NetworkManager calls it on.
    networking.networkmanager.plugins = [ cfg.package ];
    services.dbus.packages = [ cfg.package ];

    systemd.services = {
      ${earlyBoot} = mkIf cfg.enableEarlyBootBlocking {
        description = "Warren early boot network blocker";
        wantedBy = [ "${daemon}.service" ];
        before = [
          "basic.target"
          "${daemon}.service"
        ];
        unitConfig.DefaultDependencies = "no";
        serviceConfig = {
          Type = "oneshot";
          ExecStart = "${getExe' cfg.package daemon} -v --initialize-early-boot-firewall";
        };
      };

      ${daemon} = {
        description = "Warren VPN daemon";
        wantedBy = [ "multi-user.target" ];
        wants = [
          "network.target"
          "network-online.target"
        ];
        after = [
          "network-online.target"
          "NetworkManager.service"
          "systemd-resolved.service"
        ]
        ++ optional cfg.enableEarlyBootBlocking "${earlyBoot}.service";

        path = [
          # The datapath shells out to `ip` for the routes it owns.
          pkgs.iproute2
        ]
        ++ optional config.networking.resolvconf.enable config.networking.resolvconf.package;

        serviceConfig = {
          ExecStart = "${getExe' cfg.package daemon} -v --disable-stdout-timestamps";
          Restart = "always";
          RestartSec = 1;
        };
        # The daemon exits fail-closed, leaving the kernel firewall blocking, so
        # a supervisor that gave up after a burst of restarts would strand the
        # machine offline with nothing left to unblock it.
        startLimitIntervalSec = 0;
      };
    };
  };
}
