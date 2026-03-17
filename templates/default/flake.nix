{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    dregs.url = "github:EspressoSystems/dregs";
    foundry.url = "github:shazow/foundry.nix/monthly";
    solc.url = "github:EspressoSystems/nix-solc-bin";
  };

  outputs = { nixpkgs, dregs, foundry, solc, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              foundry.overlay
              solc.overlays.default
              dregs.overlays.default
            ];
          };
        in
        {
          default = pkgs.mkShellNoCC {
            buildInputs = [
              pkgs.dregs-unwrapped
              pkgs.foundry-bin
              pkgs.solc-bin."0.8.30"
            ];
          };
        }
      );
    };
}
