# A derivation for the `essential-node-api` crate.
{ lib
, stdenv
, darwin
, pkg-config
, rustPlatform
, sqlite
, openssl
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
  crateSubdir = "crates/node-cli";
  crateDir = "${src}/${crateSubdir}";
  crateTOML = "${crateDir}/Cargo.toml";
  lockFile = "${src}/Cargo.lock";
in
rustPlatform.buildRustPackage {
  inherit src;
  pname = "essential-node";
  version = (builtins.fromTOML (builtins.readFile crateTOML)).package.version;

  buildAndTestSubdir = crateSubdir;

  OPENSSL_NO_VENDOR = 1;

  nativeBuildInputs = lib.optionals stdenv.isLinux [
    pkg-config
  ];

  buildInputs = [
    sqlite
    openssl
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
