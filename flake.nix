{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";
  inputs.git-hooks.url = "github:cachix/git-hooks.nix";
  inputs.solc.url = "github:EspressoSystems/nix-solc-bin";

  outputs = { self, nixpkgs, rust-overlay, git-hooks, solc }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      checks = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              rust-overlay.overlays.default
              solc.overlays.default
            ];
          };
          rustToolchain = pkgs.rust-bin.stable.latest.default;
        in
        {
          pre-commit-check = git-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              rustfmt.enable = true;
              clippy = {
                enable = true;
                packageOverrides.cargo = rustToolchain;
                packageOverrides.clippy = rustToolchain;
              };
              cargo-test = {
                enable = true;
                name = "cargo nextest";
                entry = "${pkgs.cargo-nextest}/bin/cargo-nextest nextest run";
                files = "\\.rs$";
                pass_filenames = false;
              };
            };
          };
        }
      );

      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              rust-overlay.overlays.default
              solc.overlays.default
            ];
          };
          rustToolchain = pkgs.rust-bin.stable.latest.default;
        in
        {
          default = pkgs.mkShellNoCC {
            inherit (self.checks.${system}.pre-commit-check) shellHook;
            buildInputs = [
              rustToolchain
              pkgs.cargo-nextest
              pkgs.foundry
              pkgs.solc-bin."0.8.30"
            ];
          };
        }
      );
    };
}
