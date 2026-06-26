{
  description = "Bezel - trackpad edge gesture daemon";

  inputs = {
    nixpkgs.url = "github:yzhou216/nixpkgs?ref=bezel-init"; # TODO: wait for merge into Nixpkgs master
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        # Build with `$ nix build .#default`
        packages.default = pkgs.bezel.overrideAttrs (old: {
          pname = "bezel";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src = ./.;
          cargoHash = "";
          cargoSha256 = "";
          cargoDeps = pkgs.rustPlatform.importCargoLock {
            lockFile = ./Cargo.lock;
          };
        });
      }
    );
}
