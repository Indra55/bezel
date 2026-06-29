{
  self,
  ...
}:
{
  config,
  pkgs,
  lib,
  ...
}:

let
  cfg = config.services.bezel;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.services.bezel = {
    enable = lib.mkEnableOption "Enable Bezel Trackpad Gestures for this user only";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "The bezel package to use";
    };

    config = lib.mkOption {
      inherit (tomlFormat) type;
      default = { };
      description = ''
        Configuration written to {file}`$XDG_CONFIG_HOME/bezel/config.toml`.
        See <https://github.com/Indra55/bezel/blob/main/config.nix.example> for the full list of options.
      '';
    };
  };

  config = {
    home.packages = lib.optional (cfg.enable && cfg.package != null) cfg.package;

    systemd.user.services.bezel = lib.mkIf cfg.enable {
      Unit = {
        Description = "Bezel Trackpad Gestures";
        After = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = lib.getExe cfg.package;
        Restart = "always";
        RestartSec = 3;
      };
      Install = {
        WantedBy = [ "default.target" ];
      };
    };

    xdg.configFile."bezel/config.toml" = lib.mkIf (cfg.config != { }) {
      source = tomlFormat.generate "bezel-config" cfg.config;
    };
  };
}
