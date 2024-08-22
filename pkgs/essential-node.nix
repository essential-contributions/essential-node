# A derivation for the `essential-node-api` crate.
{ lib
, stdenv
, darwin
, pkg-config
, rustPlatform
, sqlite
}:
let
  src = builtins.path {
    path = ../.;
    filter = path: type:
      let
        keepFiles = [
          "Cargo.lock"
          "Cargo.toml"
          "crates"
        ];
        includeDirs = [
          "crates"
        ];
        isPathInIncludeDirs = dir: lib.strings.hasInfix dir path;
      in
      if lib.lists.any (p: p == (baseNameOf path)) keepFiles then
        true
      else
        lib.lists.any (dir: isPathInIncludeDirs dir) includeDirs
    ;
  };
  crateDir = "${src}/crates/node-api";
  crateTOML = "${crateDir}/Cargo.toml";
  lockFile = "${src}/Cargo.lock";
in
rustPlatform.buildRustPackage {
  inherit src;
  pname = "essential-node";
  version = (builtins.fromTOML (builtins.readFile crateTOML)).package.version;

  buildAndTestSubdir = "crates/node-cli";

  nativeBuildInputs = lib.optionals stdenv.isLinux [
    pkg-config
  ];

  buildInputs = [
    sqlite
  ] ++ lib.optionals stdenv.isLinux [
  ] ++ lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];

  # We run tests separately in CI.
  doCheck = false;

  cargoLock = {
    inherit lockFile;
  };
}
