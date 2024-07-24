{
  description = ''
    A nix flake for the essential node.
  '';

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs = inputs:
    let
      overlays = [
        inputs.self.overlays.default
      ];
      perSystemPkgs = f:
        inputs.nixpkgs.lib.genAttrs (import inputs.systems)
          (system: f (import inputs.nixpkgs { inherit overlays system; }));
    in
    {
      overlays = {
        essential-server = import ./overlay.nix { };
        default = inputs.self.overlays.essential-server;
      };

      packages = perSystemPkgs (pkgs: {
        essential-node-api = pkgs.essential-node-api;
        default = inputs.self.packages.${pkgs.system}.essential-node-api;
      });

      devShells = perSystemPkgs (pkgs: {
        essential-node-dev = pkgs.callPackage ./shell.nix { };
        default = inputs.self.devShells.${pkgs.system}.essential-node-dev;
      });

      formatter = perSystemPkgs (pkgs: pkgs.nixpkgs-fmt);
    };
}
