{
  description = "Bezel - trackpad edge gesture daemon";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems =
        callback:
        nixpkgs.lib.genAttrs [
          "x86_64-linux"
          "aarch64-linux"
        ] (system: callback nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          inherit (pkgs) lib;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage (finalAttrs: {
            pname = "bezel";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            # evdev and uinput crates typically require libudev to build
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.udev ];

            meta = {
              description = "Per-edge-zone trackpad gesture daemon";
              longDescription = ''
                Bezel is a daemon that provides customizable trackpad edge
                gestures.  It intercepts raw trackpad inputs via `evdev` and
                dispatches shell commands based on directional swipes or taps
                along the edges (zones) of your trackpad.
              '';
              changelog = "https://github.com/Indra55/bezel/releases/tag/${finalAttrs.version}";
              homepage = "https://github.com/Indra55/bezel";
              license = lib.licenses.gpl3Plus;
              mainProgram = "bezel";
              platforms = lib.platforms.linux;
            };
          });
        }
      );

      nixosModules.default = import ./modules/nixos.nix { inherit self; };
      homeManagerModules.default = import ./modules/homeManager.nix { inherit self; };
    };
}
