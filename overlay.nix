# An overlay to make it easier to merge all essential-node related packages
# into nixpkgs.
{}: final: prev: {
  essential-node-api = prev.callPackage ./pkgs/essential-node-api.nix { };
}
