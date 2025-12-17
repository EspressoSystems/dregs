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
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "llvm-tools-preview" ];
          };
        in
        {
          pre-commit-check = git-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              rustfmt = {
                enable = true;
                packageOverrides.cargo = rustToolchain;
                packageOverrides.rustfmt = rustToolchain;
              };
              clippy = {
                enable = true;
                name = "clippy";
                entry = "${rustToolchain}/bin/cargo-clippy clippy --all-targets -- -D warnings";
                types_or = [ "rust" "toml" ];
                pass_filenames = false;
              };
              cargo-test = {
                enable = true;
                name = "test";
                entry = "${pkgs.cargo-nextest}/bin/cargo-nextest nextest run";
                types_or = [ "rust" "toml" ];
                pass_filenames = false;
              };
              cargo-llvm-cov = {
                enable = true;
                name = "coverage";
                entry =
                  if pkgs.stdenv.isDarwin
                  then "echo 'WARNING: cargo-llvm-cov skipped on Darwin (package broken)'"
                  else "${pkgs.cargo-llvm-cov}/bin/cargo-llvm-cov llvm-cov --fail-under-lines 100";
                types_or = [ "rust" "toml" ];
                pass_filenames = false;
              };
              spell-checking = {
                enable = true;
                name = "typos";
                entry = "${pkgs.typos}/bin/typos --force-exclude";
                pass_filenames = true;
              };
              cargo-lock = {
                enable = true;
                name = "cargo-lock";
                entry = "${rustToolchain}/bin/cargo update --workspace --verbose";
                types_or = [ "toml" ];
                pass_filenames = false;
              };
              nixpkgs-fmt.enable = true;
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
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "llvm-tools-preview" ];
          };
        in
        {
          default = pkgs.mkShellNoCC {
            inherit (self.checks.${system}.pre-commit-check) shellHook;
            buildInputs = [
              rustToolchain
              pkgs.cargo-nextest
              pkgs.foundry
              pkgs.just
              pkgs.solc-bin."0.8.30"
              pkgs.typos
              pkgs.nixpkgs-fmt
            ] ++ pkgs.lib.optionals (!pkgs.stdenv.isDarwin) [
              pkgs.cargo-llvm-cov
            ];
            RUST_BACKTRACE = 1;
          };
        }
      );
    };
}
