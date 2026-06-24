{
  description = "Bezel - trackpad edge gesture daemon";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      forAllSystems = callback:
      nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ] (system: callback nixpkgs.legacyPackages.${system});

      make-package = pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "bezel";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          # evdev and uinput crates typically require libudev to build
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.udev ];
        };
      };
    in {
      packages = forAllSystems make-package;
    };
}
