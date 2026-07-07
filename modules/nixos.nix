{
  self,
  ...
}:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.bezel;
in
{
  options.services.bezel = {
    enable = lib.mkEnableOption "Enable the Bezel Trackpad Gestures service for all users";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "The bezel package to use";
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = with pkgs; [
        wireplumber
        brightnessctl
        playerctl
        # hyprland
      ];
      description = "Extra packages to add to the systemd unit's PATH. By default installs packages required by the default configuration (except for Hyprland)";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = lib.optional (cfg.package != null) cfg.package;

    systemd.user.services.bezel = {
      description = "Bezel Trackpad Gestures";
      after = [ "graphical-session.target" ];
      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "always";
        RestartSec = 3;
      };
      path = [ pkgs.bash ] ++ cfg.extraPackages;
      wantedBy = [ "default.target" ];
    };

    hardware.uinput.enable = true;
  };
}
